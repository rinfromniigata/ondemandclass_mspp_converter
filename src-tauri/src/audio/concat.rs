//! pptx内の音声を取り出して無音区間とともに1本の m4a に結合し、各スライドの区間を求める。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use super::plan::{decide_mode, ConcatMode};
use super::probe::{probe, AudioInfo};
use super::progress::{parse_out_time_sec, ConcatProgress, ProgressSink, StageWeights};
use super::timeline::build_timeline;
use super::{AudioSegment, ConcatResult, PROCESS_TIMEOUT};
use crate::error::AppError;
use crate::fs_util::move_or_copy;
use crate::pptx::package::PptxPackage;
use crate::process::{run_with_timeout, run_with_timeout_streaming};

/// 再エンコード方式で正規化するサンプルレート
const REENCODE_SAMPLE_RATE: u32 = 48000;
/// 再エンコード方式の出力ビットレート
const REENCODE_BITRATE: &str = "192k";
/// 一時フォルダ内の結合結果のファイル名
const OUTPUT_NAME: &str = "output.m4a";

pub const REENCODE_DISABLED_MESSAGE: &str = "音声の形式がスライド間で異なります。app.settings.json の audioReencodeOnMismatch を true にすると再エンコードで結合できます";

/// 使用する外部ツールのパス
pub struct FfmpegTools<'a> {
    pub ffmpeg: &'a Path,
    pub ffprobe: &'a Path,
}

/// 一時フォルダに置いた区間1つ分の素材
enum Part {
    Media {
        slide_index: u32,
        media_path: String,
        file: PathBuf,
        info: AudioInfo,
    },
    Silence {
        slide_index: u32,
        duration_sec: f64,
    },
}

/// `segments` を順に結合して `out_path` に書き出し、`slide_indices` の全スライドの区間を返す。
///
/// 作業はすべて一時フォルダ（関数を抜けるとDropで削除）で行い、最後に結果だけを `out_path` へ移す。
/// 途中で失敗しても既存の `out_path` は壊れない。
///
/// 進捗は 0〜1 の割合で `on_progress` に通知する（単調増加・1%未満の増加は省く）。1 は出力を保存し終えたときだけ送る
pub fn concat_audio(
    tools: &FfmpegTools<'_>,
    input: &Path,
    slide_indices: &[u32],
    segments: &[AudioSegment],
    out_path: &Path,
    reencode_on_mismatch: bool,
    on_progress: ProgressSink<'_>,
) -> Result<ConcatResult, AppError> {
    validate_segments(slide_indices, segments).map_err(AppError::Message)?;

    let mut progress = ConcatProgress::new(on_progress);
    let tmp = TempDir::new().map_err(|source| AppError::Io {
        context: "一時フォルダを作成できません".into(),
        source,
    })?;
    let dir = tmp.path();
    let parts = extract_parts(tools, input, segments, dir, &mut progress)?;
    let infos: Vec<AudioInfo> = parts
        .iter()
        .filter_map(|part| match part {
            Part::Media { info, .. } => Some(info.clone()),
            Part::Silence { .. } => None,
        })
        .collect();
    let output = dir.join(OUTPUT_NAME);

    let (durations, reencoded) = match decide_mode(&infos) {
        ConcatMode::Copy {
            sample_rate,
            channels,
            ..
        } => {
            progress.start_mode(StageWeights::COPY);
            match concat_copy(
                tools,
                &parts,
                sample_rate,
                channels,
                dir,
                &output,
                &mut progress,
            ) {
                Ok(durations) => (durations, false),
                // copy 結合で ffmpeg が非ゼロ終了した場合だけ再エンコードで再試行する
                Err(AppError::ProcessFailed { .. }) if reencode_on_mismatch => {
                    progress.restart_with(StageWeights::REENCODE);
                    let channels = channels.min(2);
                    (
                        concat_reencode(tools, &parts, channels, dir, &output, &mut progress)?,
                        true,
                    )
                }
                Err(e) => return Err(e),
            }
        }
        ConcatMode::Reencode { channels } => {
            if !reencode_on_mismatch {
                return Err(AppError::Message(REENCODE_DISABLED_MESSAGE.into()));
            }
            progress.start_mode(StageWeights::REENCODE);
            (
                concat_reencode(tools, &parts, channels, dir, &output, &mut progress)?,
                true,
            )
        }
    };

    let timestamps = build_timeline(slide_indices, &durations).map_err(AppError::Message)?;
    move_or_copy(&output, out_path).map_err(|source| AppError::Io {
        context: format!("結合した音声を保存できません（{}）", out_path.display()),
        source,
    })?;
    progress.finish();
    Ok(ConcatResult {
        timestamps,
        reencoded,
    })
}

/// 外部プロセスを起動する前に、区間列の形を確かめる（純粋関数）
fn validate_segments(slide_indices: &[u32], segments: &[AudioSegment]) -> Result<(), String> {
    if segments.is_empty() {
        return Err("結合する音声区間がありません".into());
    }
    for segment in segments {
        if let AudioSegment::Silence {
            slide_index,
            duration_sec,
        } = segment
        {
            if !duration_sec.is_finite() || *duration_sec <= 0.0 {
                return Err(format!(
                    "スライド{slide_index}の無音区間の長さが不正です（{duration_sec}秒）"
                ));
            }
        }
    }
    // 並び順だけを確かめる（長さは結合後に決まる）
    let order: Vec<(u32, f64)> = segments.iter().map(|s| (s.slide_index(), 0.0)).collect();
    build_timeline(slide_indices, &order).map(|_| ())
}

/// `media` 区間の音声を pptx から一時フォルダへ書き出し、ffprobe で形式と長さを調べる。
/// 区間を1つ終えるごとに取り出しの進捗を進める
fn extract_parts(
    tools: &FfmpegTools<'_>,
    input: &Path,
    segments: &[AudioSegment],
    dir: &Path,
    progress: &mut ConcatProgress<'_>,
) -> Result<Vec<Part>, AppError> {
    // 音声区間がなければ pptx は開かない
    let mut pkg = None;
    let mut parts = Vec::with_capacity(segments.len());
    for (i, segment) in segments.iter().enumerate() {
        let part = match segment {
            AudioSegment::Media {
                slide_index,
                media_path,
            } => {
                let in_media = media_error(*slide_index, media_path);
                let pkg = match &mut pkg {
                    Some(pkg) => pkg,
                    None => pkg.insert(PptxPackage::open(input)?),
                };
                let bytes = pkg.read_bytes(media_path).map_err(&in_media)?;
                let file = dir.join(media_part_name(i, media_path));
                fs::write(&file, bytes).map_err(|source| AppError::Io {
                    context: format!("音声を一時フォルダへ書き出せません（{media_path}）"),
                    source,
                })?;
                let info = probe(tools.ffprobe, &file).map_err(&in_media)?;
                Part::Media {
                    slide_index: *slide_index,
                    media_path: media_path.clone(),
                    file,
                    info,
                }
            }
            AudioSegment::Silence {
                slide_index,
                duration_sec,
            } => Part::Silence {
                slide_index: *slide_index,
                duration_sec: *duration_sec,
            },
        };
        parts.push(part);
        progress.extracted(i + 1, segments.len());
    }
    Ok(parts)
}

/// 無音を音声と同じ形式（AAC・サンプルレート・チャンネル数）で生成し、`-c copy` で結合する。
/// 区間の長さは、音声は ffprobe の値、無音は指定秒
fn concat_copy(
    tools: &FfmpegTools<'_>,
    parts: &[Part],
    sample_rate: u32,
    channels: u32,
    dir: &Path,
    output: &Path,
    progress: &mut ConcatProgress<'_>,
) -> Result<Vec<(u32, f64)>, AppError> {
    let mut names = Vec::with_capacity(parts.len());
    let mut durations = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        match part {
            Part::Media {
                slide_index,
                file,
                info,
                ..
            } => {
                names.push(file_name_of(file));
                durations.push((*slide_index, info.duration_sec));
            }
            Part::Silence {
                slide_index,
                duration_sec,
            } => {
                let name = format!("part_{i}.m4a");
                run_ffmpeg(tools.ffmpeg, |cmd| {
                    cmd.args(silence_input_args(sample_rate, channels, *duration_sec))
                        .args(["-c:a", "aac"])
                        .arg(dir.join(&name));
                })
                .map_err(|e| silence_error(*slide_index, e))?;
                names.push(name);
                durations.push((*slide_index, *duration_sec));
            }
        }
        progress.generated(i + 1, parts.len());
    }

    let list = write_concat_list(dir, "list.txt", &names)?;
    run_ffmpeg_with_progress(tools.ffmpeg, total_sec(&durations), progress, |cmd| {
        cmd.args(["-f", "concat", "-safe", "0", "-i"])
            .arg(&list)
            .args(["-vn", "-c", "copy", "-movflags", "+faststart"])
            .arg(output);
    })?;
    Ok(durations)
}

/// 各区間を 48kHz・`channels` チャンネルの PCM WAV に正規化してから AAC で結合する。
/// 区間の長さは、正規化後のファイルを ffprobe で測り直した値
fn concat_reencode(
    tools: &FfmpegTools<'_>,
    parts: &[Part],
    channels: u32,
    dir: &Path,
    output: &Path,
    progress: &mut ConcatProgress<'_>,
) -> Result<Vec<(u32, f64)>, AppError> {
    let channels_arg = channels.to_string();
    let rate_arg = REENCODE_SAMPLE_RATE.to_string();
    let mut names = Vec::with_capacity(parts.len());
    let mut durations = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        let name = format!("norm_{i}.wav");
        let norm = dir.join(&name);
        let (slide_index, info) = match part {
            Part::Media {
                slide_index,
                media_path,
                file,
                ..
            } => {
                let in_media = media_error(*slide_index, media_path);
                run_ffmpeg(tools.ffmpeg, |cmd| {
                    cmd.arg("-i")
                        .arg(file)
                        .args(["-map", "0:a:0", "-ar", &rate_arg, "-ac", &channels_arg])
                        .args(["-c:a", "pcm_s16le"])
                        .arg(&norm);
                })
                .map_err(&in_media)?;
                (
                    *slide_index,
                    probe(tools.ffprobe, &norm).map_err(&in_media)?,
                )
            }
            Part::Silence {
                slide_index,
                duration_sec,
            } => {
                let in_silence = |e| silence_error(*slide_index, e);
                run_ffmpeg(tools.ffmpeg, |cmd| {
                    cmd.args(silence_input_args(
                        REENCODE_SAMPLE_RATE,
                        channels,
                        *duration_sec,
                    ))
                    .args(["-c:a", "pcm_s16le"])
                    .arg(&norm);
                })
                .map_err(in_silence)?;
                (
                    *slide_index,
                    probe(tools.ffprobe, &norm).map_err(in_silence)?,
                )
            }
        };
        names.push(name);
        durations.push((slide_index, info.duration_sec));
        progress.generated(i + 1, parts.len());
    }

    let list = write_concat_list(dir, "list_norm.txt", &names)?;
    run_ffmpeg_with_progress(tools.ffmpeg, total_sec(&durations), progress, |cmd| {
        cmd.args(["-f", "concat", "-safe", "0", "-i"])
            .arg(&list)
            .args(["-c:a", "aac", "-b:a", REENCODE_BITRATE])
            .args(["-movflags", "+faststart"])
            .arg(output);
    })?;
    Ok(durations)
}

/// `ffmpeg -y -v error <configure で追加した引数>` を実行し、非ゼロ終了をエラーにする
fn run_ffmpeg(ffmpeg: &Path, configure: impl FnOnce(&mut Command)) -> Result<(), AppError> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-v", "error"]);
    configure(&mut cmd);
    run_with_timeout(cmd, PROCESS_TIMEOUT)?.ensure_success(ffmpeg)?;
    Ok(())
}

/// `run_ffmpeg` に `-progress pipe:1 -nostats` を加えて実行し、stdout の `out_time_us` で結合の進捗を進める。
/// `total_sec` は出力の総尺の見込み（各区間の長さの合計）
fn run_ffmpeg_with_progress(
    ffmpeg: &Path,
    total_sec: f64,
    progress: &mut ConcatProgress<'_>,
    configure: impl FnOnce(&mut Command),
) -> Result<(), AppError> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-v", "error", "-progress", "pipe:1", "-nostats"]);
    configure(&mut cmd);
    let mut on_line = |line: &str| {
        if let Some(out_sec) = parse_out_time_sec(line) {
            progress.concatenating(out_sec, total_sec);
        }
    };
    run_with_timeout_streaming(cmd, PROCESS_TIMEOUT, &mut on_line)?.ensure_success(ffmpeg)?;
    Ok(())
}

/// 区間の長さの合計（秒）
fn total_sec(durations: &[(u32, f64)]) -> f64 {
    durations.iter().map(|(_, sec)| sec).sum()
}

/// anullsrc で `duration_sec` 秒の無音を入力にする引数
fn silence_input_args(sample_rate: u32, channels: u32, duration_sec: f64) -> [String; 6] {
    [
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        format!("anullsrc=r={sample_rate}:cl={}", channel_layout(channels)),
        "-t".into(),
        duration_sec.to_string(),
    ]
}

/// anullsrc の `cl` に渡すチャンネルレイアウト。3ch以上は `{n}c`（ffmpeg 9.0.2 で 2.1・5.1 等になることを確認）
fn channel_layout(channels: u32) -> String {
    match channels {
        1 => "mono".into(),
        2 => "stereo".into(),
        n => format!("{n}c"),
    }
}

/// 一時フォルダに書き出す音声のファイル名（`part_{i}.{元の拡張子}`）。
/// 拡張子が英数字でなければ `bin` にする（concat のリストに引用符等を入れないため）
fn media_part_name(i: usize, media_path: &str) -> String {
    let ext = Path::new(media_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "bin".into());
    format!("part_{i}.{ext}")
}

/// concat demuxer のリスト。ファイル名はリストと同じフォルダからの相対名
/// （一時フォルダのパスに含まれうる引用符等のエスケープを避けるため）
fn concat_list(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("file '{name}'\n"))
        .collect()
}

fn write_concat_list(dir: &Path, list_name: &str, names: &[String]) -> Result<PathBuf, AppError> {
    let list = dir.join(list_name);
    fs::write(&list, concat_list(names)).map_err(|source| AppError::Io {
        context: "結合リストを作成できません".into(),
        source,
    })?;
    Ok(list)
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn media_error(slide_index: u32, media_path: &str) -> impl Fn(AppError) -> AppError + '_ {
    move |e| AppError::Message(format!("スライド{slide_index}の音声（{media_path}）：{e}"))
}

fn silence_error(slide_index: u32, e: AppError) -> AppError {
    AppError::Message(format!(
        "スライド{slide_index}の無音区間を生成できません：{e}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(slide_index: u32, path: &str) -> AudioSegment {
        AudioSegment::Media {
            slide_index,
            media_path: path.into(),
        }
    }

    fn silence(slide_index: u32, duration_sec: f64) -> AudioSegment {
        AudioSegment::Silence {
            slide_index,
            duration_sec,
        }
    }

    #[test]
    fn validate_accepts_ordered_segments() {
        let segments = [
            silence(1, 3.0),
            media(2, "ppt/media/media1.m4a"),
            media(2, "ppt/media/media2.m4a"),
        ];
        assert_eq!(validate_segments(&[1, 2, 3], &segments), Ok(()));
    }

    #[test]
    fn validate_rejects_empty_segments() {
        assert_eq!(
            validate_segments(&[1], &[]),
            Err("結合する音声区間がありません".into())
        );
    }

    #[test]
    fn validate_rejects_invalid_silence_duration() {
        for sec in [0.0, -1.0, f64::NAN] {
            let err = validate_segments(&[1], &[silence(1, sec)]).unwrap_err();
            assert!(
                err.starts_with("スライド1の無音区間の長さが不正です"),
                "{err}"
            );
        }
    }

    #[test]
    fn validate_rejects_out_of_order_segments() {
        let segments = [media(2, "ppt/media/media1.m4a"), silence(1, 1.0)];
        assert_eq!(
            validate_segments(&[1, 2], &segments),
            Err("スライド1の音声区間が、スライドの表示順に並んでいません".into())
        );
    }

    #[test]
    fn channel_layout_names() {
        assert_eq!(channel_layout(1), "mono");
        assert_eq!(channel_layout(2), "stereo");
        assert_eq!(channel_layout(6), "6c");
    }

    #[test]
    fn silence_args_use_anullsrc() {
        assert_eq!(
            silence_input_args(44100, 2, 2.5),
            [
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=44100:cl=stereo",
                "-t",
                "2.5"
            ]
            .map(String::from)
        );
        // 小さい値でも指数表記にならない
        assert_eq!(silence_input_args(48000, 1, 0.0001)[5], "0.0001");
    }

    #[test]
    fn media_part_name_keeps_safe_extension() {
        assert_eq!(media_part_name(0, "ppt/media/media1.m4a"), "part_0.m4a");
        assert_eq!(media_part_name(3, "ppt/media/Media2.MP3"), "part_3.mp3");
        assert_eq!(media_part_name(1, "ppt/media/media3"), "part_1.bin");
        assert_eq!(media_part_name(2, "ppt/media/x.m'4a"), "part_2.bin");
    }

    #[test]
    fn concat_list_uses_relative_names() {
        let names = ["part_0.m4a".to_string(), "part_1.m4a".to_string()];
        assert_eq!(
            concat_list(&names),
            "file 'part_0.m4a'\nfile 'part_1.m4a'\n"
        );
    }

    #[test]
    fn concat_audio_checks_segments_before_running_tools() {
        let tools = FfmpegTools {
            ffmpeg: Path::new("missing-ffmpeg.exe"),
            ffprobe: Path::new("missing-ffprobe.exe"),
        };
        let err = concat_audio(
            &tools,
            Path::new("missing.pptx"),
            &[1],
            &[],
            Path::new("out.m4a"),
            true,
            &mut |_| {},
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "結合する音声区間がありません");
    }
}
