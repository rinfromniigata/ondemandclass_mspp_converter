//! 実際の ffmpeg / ffprobe を使う音声結合の結合テスト（imple 6.1）。
//! 環境変数 `FFMPEG_PATH` / `FFPROBE_PATH` に実行ファイルのパスを設定し、
//! `cargo test --test concat_integration -- --ignored` で実行する
//!
//! `concat_audio` の一時フォルダが残らないことを確かめるため、tempfile の既定の一時フォルダを
//! テスト専用のフォルダに差し替え、結合の呼び出しは1つずつ（ロックで直列に）行う

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use common::fixture::{PptxBuilder, SlideBuilder};
use ondemandclass_mspp_converter_lib::audio::concat::{
    concat_audio, FfmpegTools, REENCODE_DISABLED_MESSAGE,
};
use ondemandclass_mspp_converter_lib::audio::probe::{probe, AudioInfo};
use ondemandclass_mspp_converter_lib::audio::{AudioSegment, ConcatResult, SlideTimestampEntry};
use ondemandclass_mspp_converter_lib::error::AppError;
use tempfile::TempDir;

/// 結合結果の長さと timestamp の終端の許容差（秒）
const DURATION_TOLERANCE: f64 = 0.05;

struct TestEnv {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    /// `concat_audio` が一時フォルダを作る場所（tempfile の既定を差し替えたもの）
    work_dir: PathBuf,
    /// 生成した音声の置き場所（テスト間で共有）
    audio_dir: PathBuf,
    /// テストごとの入出力フォルダの親
    case_root: PathBuf,
}

fn env() -> &'static TestEnv {
    static ENV: OnceLock<TestEnv> = OnceLock::new();
    ENV.get_or_init(|| {
        let tool = |name: &str| {
            PathBuf::from(std::env::var(name).unwrap_or_else(|_| {
                panic!("環境変数 {name} に実行ファイルのパスを設定してください")
            }))
        };
        let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("concat_integration");
        let _ = fs::remove_dir_all(&base);
        let env = TestEnv {
            ffmpeg: tool("FFMPEG_PATH"),
            ffprobe: tool("FFPROBE_PATH"),
            work_dir: base.join("work"),
            audio_dir: base.join("audio"),
            case_root: base.join("cases"),
        };
        for dir in [&env.work_dir, &env.audio_dir, &env.case_root] {
            fs::create_dir_all(dir).unwrap();
        }
        tempfile::env::override_temp_dir(&env.work_dir)
            .expect("tempfile の一時フォルダは、このテストより前に差し替えられていないこと");
        env
    })
}

/// `concat_audio` の呼び出しを直列にする（一時フォルダが空かどうかを確かめるため）
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn tools(env: &TestEnv) -> FfmpegTools<'_> {
    FfmpegTools {
        ffmpeg: &env.ffmpeg,
        ffprobe: &env.ffprobe,
    }
}

#[derive(Clone, Copy)]
enum Codec {
    Aac,
    Mp3,
}

/// ffmpeg で正弦波の音声を作り、バイト列を返す（同じ条件のファイルは使い回す）
fn tone(
    env: &TestEnv,
    codec: Codec,
    sample_rate: u32,
    channels: u32,
    duration_sec: f64,
) -> Vec<u8> {
    let (encoder, ext) = match codec {
        Codec::Aac => ("aac", "m4a"),
        Codec::Mp3 => ("libmp3lame", "mp3"),
    };
    let path = env.audio_dir.join(format!(
        "{encoder}_{sample_rate}_{channels}_{duration_sec}.{ext}"
    ));
    // 並行するテストが書きかけのファイルを読まないよう、生成と読み取りを直列にする
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if !path.exists() {
        let status = Command::new(&env.ffmpeg)
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg(format!(
                "sine=frequency=440:sample_rate={sample_rate}:duration={duration_sec}"
            ))
            .args(["-ac", &channels.to_string(), "-c:a", encoder])
            .arg(&path)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "テスト用の音声を生成できません（{path:?}）"
        );
    }
    fs::read(path).unwrap()
}

/// 音声1つのスライド（rId2 → ../media/{media}）
fn narrated(media: &str) -> SlideBuilder {
    SlideBuilder::new()
        .text(2)
        .audio(4, "rId2")
        .timing_audio(&[4])
        .audio_rel("rId2", &format!("../media/{media}"))
}

fn media(slide_index: u32, name: &str) -> AudioSegment {
    AudioSegment::Media {
        slide_index,
        media_path: format!("ppt/media/{name}"),
    }
}

fn silence(slide_index: u32, duration_sec: f64) -> AudioSegment {
    AudioSegment::Silence {
        slide_index,
        duration_sec,
    }
}

/// 1ケース分の入力pptxと出力先
struct Case {
    _dir: TempDir,
    input: PathBuf,
    out: PathBuf,
}

fn case(env: &TestEnv, builder: &PptxBuilder) -> Case {
    let dir = TempDir::new_in(&env.case_root).unwrap();
    let input = builder.write_to(dir.path(), "lecture");
    let out = dir.path().join("lecture_audio.m4a");
    Case {
        _dir: dir,
        input,
        out,
    }
}

/// 直列に `concat_audio` を実行し、一時フォルダが残っていないことを確かめる
fn run(
    env: &TestEnv,
    case: &Case,
    slide_indices: &[u32],
    segments: &[AudioSegment],
    reencode_on_mismatch: bool,
) -> Result<ConcatResult, AppError> {
    let _guard = serial();
    let result = concat_audio(
        &tools(env),
        &case.input,
        slide_indices,
        segments,
        &case.out,
        reencode_on_mismatch,
    );
    let leftovers: Vec<_> = fs::read_dir(&env.work_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(
        leftovers.is_empty(),
        "一時フォルダが残っています: {leftovers:?}"
    );
    result
}

fn probe_file(env: &TestEnv, path: &Path) -> AudioInfo {
    probe(&env.ffprobe, path).unwrap()
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= DURATION_TOLERANCE,
        "{what}: {actual} と {expected} の差が {DURATION_TOLERANCE} 秒を超えています"
    );
}

/// 出力が AAC で、長さが最後の endSec と一致する
fn assert_output_matches(env: &TestEnv, case: &Case, result: &ConcatResult) -> AudioInfo {
    let info = probe_file(env, &case.out);
    assert_eq!(info.codec, "aac");
    let end = result.timestamps.last().unwrap().end_sec;
    assert_close(info.duration_sec, end, "出力の長さ");
    info
}

/// 各スライドの区間が連続している（前のスライドの終了＝次のスライドの開始）
fn assert_contiguous(timestamps: &[SlideTimestampEntry]) {
    assert_eq!(timestamps[0].start_sec, 0.0);
    for pair in timestamps.windows(2) {
        assert_eq!(pair[0].end_sec, pair[1].start_sec, "{pair:?}");
    }
}

/// 形式がそろったAAC：copy 方式。先頭の無音・1枚に2区間・skip（長さ0）・既存出力の置き換え
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn copy_mode_with_silence_and_skipped_slides() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, SlideBuilder::new().text(2))
        .slide(2, narrated("a.m4a"))
        .slide(3, SlideBuilder::new().text(2))
        .slide(4, narrated("c.m4a"))
        .slide(5, SlideBuilder::new().text(2))
        .media("a.m4a", &tone(env, Codec::Aac, 44100, 2, 2.0))
        .media("b.m4a", &tone(env, Codec::Aac, 44100, 2, 1.0))
        .media("c.m4a", &tone(env, Codec::Aac, 44100, 2, 1.5));
    let case = case(env, &builder);
    fs::write(&case.out, b"old").unwrap();

    let segments = [
        silence(1, 1.5),
        media(2, "a.m4a"),
        media(2, "b.m4a"),
        media(4, "c.m4a"),
    ];
    let result = run(env, &case, &[1, 2, 3, 4, 5], &segments, true).unwrap();

    assert!(!result.reencoded);
    let t = &result.timestamps;
    assert_eq!(
        t.iter().map(|e| e.slide).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert_contiguous(t);
    // 先頭の無音は指定秒どおり
    assert_eq!((t[0].start_sec, t[0].end_sec), (0.0, 1.5));
    // スライド2は2区間の合計（AACのため実際の長さは指定より少し長くなりうる）
    assert_close(t[1].end_sec - t[1].start_sec, 3.0, "スライド2の長さ");
    // skip したスライドは長さ0
    assert_eq!(t[2].start_sec, t[2].end_sec);
    assert_close(t[3].end_sec - t[3].start_sec, 1.5, "スライド4の長さ");
    assert_eq!(t[4].start_sec, t[4].end_sec);

    let info = assert_output_matches(env, &case, &result);
    assert_eq!((info.sample_rate, info.channels), (44100, 2));
}

/// モノラルのAAC：copy 方式で、無音もモノラルで生成される
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn copy_mode_keeps_mono() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, SlideBuilder::new().text(2))
        .media("a.m4a", &tone(env, Codec::Aac, 48000, 1, 1.0));
    let case = case(env, &builder);

    let result = run(
        env,
        &case,
        &[1, 2],
        &[media(1, "a.m4a"), silence(2, 2.0)],
        true,
    )
    .unwrap();

    assert!(!result.reencoded);
    assert_close(
        result.timestamps[1].end_sec - result.timestamps[1].start_sec,
        2.0,
        "無音の長さ",
    );
    let info = assert_output_matches(env, &case, &result);
    assert_eq!((info.sample_rate, info.channels), (48000, 1));
}

/// 3ch以上（5.1ch）のAAC：copy 方式で、無音も同じチャンネル数で生成される
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn copy_mode_with_multichannel_silence() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, SlideBuilder::new().text(2))
        .media("a.m4a", &tone(env, Codec::Aac, 48000, 6, 1.0));
    let case = case(env, &builder);

    let result = run(
        env,
        &case,
        &[1, 2],
        &[media(1, "a.m4a"), silence(2, 1.0)],
        true,
    )
    .unwrap();

    assert!(!result.reencoded);
    let info = assert_output_matches(env, &case, &result);
    assert_eq!(info.channels, 6);
}

/// サンプルレート不一致：再エンコード方式（48kHz）
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn reencodes_on_sample_rate_mismatch() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, SlideBuilder::new().text(2))
        .slide(3, narrated("b.m4a"))
        .media("a.m4a", &tone(env, Codec::Aac, 44100, 2, 1.0))
        .media("b.m4a", &tone(env, Codec::Aac, 48000, 2, 1.0));
    let case = case(env, &builder);

    let segments = [media(1, "a.m4a"), silence(2, 2.0), media(3, "b.m4a")];
    let result = run(env, &case, &[1, 2, 3], &segments, true).unwrap();

    assert!(result.reencoded);
    assert_contiguous(&result.timestamps);
    // 無音は48kHzのWAVに正規化した長さ（サンプル単位で正確）
    let t = &result.timestamps[1];
    assert_close(t.end_sec - t.start_sec, 2.0, "無音の長さ");
    let info = assert_output_matches(env, &case, &result);
    assert_eq!((info.sample_rate, info.channels), (48000, 2));
}

/// チャンネル数不一致：再エンコード方式で、最大チャンネル数（ステレオ）にそろえる
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn reencodes_on_channel_mismatch() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, narrated("b.m4a"))
        .media("a.m4a", &tone(env, Codec::Aac, 48000, 1, 1.0))
        .media("b.m4a", &tone(env, Codec::Aac, 48000, 2, 1.0));
    let case = case(env, &builder);

    let result = run(
        env,
        &case,
        &[1, 2],
        &[media(1, "a.m4a"), media(2, "b.m4a")],
        true,
    )
    .unwrap();

    assert!(result.reencoded);
    let info = assert_output_matches(env, &case, &result);
    assert_eq!((info.sample_rate, info.channels), (48000, 2));
}

/// MP3混在：再エンコード方式
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn reencodes_when_mp3_is_mixed() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, narrated("b.mp3"))
        .media("a.m4a", &tone(env, Codec::Aac, 44100, 2, 1.0))
        .media("b.mp3", &tone(env, Codec::Mp3, 44100, 2, 1.0));
    let case = case(env, &builder);

    let result = run(
        env,
        &case,
        &[1, 2],
        &[media(1, "a.m4a"), media(2, "b.mp3")],
        true,
    )
    .unwrap();

    assert!(result.reencoded);
    assert_contiguous(&result.timestamps);
    assert_close(
        result.timestamps[1].end_sec - result.timestamps[1].start_sec,
        1.0,
        "MP3の長さ",
    );
    let info = assert_output_matches(env, &case, &result);
    assert_eq!((info.sample_rate, info.channels), (48000, 2));
}

/// 再エンコードが必要で audioReencodeOnMismatch = false：エラーにし、既存の出力を壊さない
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn mismatch_is_error_when_reencode_disabled() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, narrated("b.m4a"))
        .media("a.m4a", &tone(env, Codec::Aac, 44100, 2, 1.0))
        .media("b.m4a", &tone(env, Codec::Aac, 48000, 2, 1.0));
    let case = case(env, &builder);
    fs::write(&case.out, b"old").unwrap();

    let err = run(
        env,
        &case,
        &[1, 2],
        &[media(1, "a.m4a"), media(2, "b.m4a")],
        false,
    )
    .unwrap_err();

    assert_eq!(err.to_string(), REENCODE_DISABLED_MESSAGE);
    assert_eq!(fs::read(&case.out).unwrap(), b"old");
}

/// pptx内に実体がない音声：スライド番号とパート名を含むエラーにし、出力を作らない
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn missing_media_is_error_with_slide_number() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, narrated("missing.m4a"))
        .media("a.m4a", &tone(env, Codec::Aac, 44100, 2, 1.0));
    let case = case(env, &builder);

    let err = run(
        env,
        &case,
        &[1, 2],
        &[media(1, "a.m4a"), media(2, "missing.m4a")],
        true,
    )
    .unwrap_err();

    let message = err.to_string();
    assert!(
        message.starts_with("スライド2の音声（ppt/media/missing.m4a）："),
        "{message}"
    );
    assert!(!case.out.exists());
}

/// 音声として読めないメディア：ffprobe の失敗をスライド番号付きで返す
#[test]
#[ignore = "ffmpeg/ffprobe が必要（FFMPEG_PATH / FFPROBE_PATH）"]
fn unreadable_media_is_error_with_slide_number() {
    let env = env();
    let builder = PptxBuilder::new()
        .slide(1, narrated("broken.m4a"))
        .media("broken.m4a", b"not audio");
    let case = case(env, &builder);

    let err = run(env, &case, &[1], &[media(1, "broken.m4a")], true).unwrap_err();

    let message = err.to_string();
    assert!(
        message.starts_with("スライド1の音声（ppt/media/broken.m4a）："),
        "{message}"
    );
    assert!(!case.out.exists());
}
