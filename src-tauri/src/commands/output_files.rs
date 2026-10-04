use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

use crate::audio::SlideTimestampEntry;
use crate::error::AppError;

/// `paths` のうち既に存在するものを、渡された順に返す（上書き確認用）
#[tauri::command]
pub async fn check_outputs_exist(paths: Vec<String>) -> Vec<String> {
    existing_paths(paths)
}

/// `entries` を timestamps JSON に整形し、UTF-8 で `out_path` に書き出して `out_path` を返す
#[tauri::command]
pub async fn write_timestamps_json(
    out_path: String,
    entries: Vec<SlideTimestampEntry>,
) -> Result<String, String> {
    let json = format_timestamps_json(&entries)?;
    write_atomically(Path::new(&out_path), &json).map_err(|e| e.to_string())?;
    Ok(out_path)
}

fn existing_paths(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|path| Path::new(path).exists())
        .collect()
}

/// 秒を小数点以下3桁に丸めた timestamps JSON（スペック7章のスキーマ。末尾に改行）を作る（純粋関数）。
/// 秒が有限でない場合はエラー（serde_json は NaN を null にしてしまうため）
fn format_timestamps_json(entries: &[SlideTimestampEntry]) -> Result<String, String> {
    if let Some(entry) = entries
        .iter()
        .find(|e| !e.start_sec.is_finite() || !e.end_sec.is_finite())
    {
        return Err(format!(
            "スライド{}の時刻が不正です（{}〜{}秒）",
            entry.slide, entry.start_sec, entry.end_sec
        ));
    }
    let rounded: Vec<SlideTimestampEntry> = entries
        .iter()
        .map(|e| SlideTimestampEntry {
            slide: e.slide,
            start_sec: round_millis(e.start_sec),
            end_sec: round_millis(e.end_sec),
        })
        .collect();
    let mut json = serde_json::to_string_pretty(&rounded)
        .map_err(|e| format!("timestamps JSON を作成できません（{e}）"))?;
    json.push('\n');
    Ok(json)
}

/// 小数点以下3桁に丸める。`-0.0` は `0.0` にする（JSONに `-0.0` と出さないため）
fn round_millis(sec: f64) -> f64 {
    (sec * 1000.0).round() / 1000.0 + 0.0
}

/// 同じフォルダの一時ファイルに書いてから `out_path` へ置き換える。
/// 途中で失敗しても既存の `out_path` は壊れず、一時ファイルは削除される
fn write_atomically(out_path: &Path, text: &str) -> Result<(), AppError> {
    let io_error = |source| AppError::Io {
        context: format!("ファイルを書き出せません（{}）", out_path.display()),
        source,
    };
    let dir = match out_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut tmp = NamedTempFile::new_in(dir).map_err(io_error)?;
    tmp.write_all(text.as_bytes()).map_err(io_error)?;
    tmp.as_file().sync_all().map_err(io_error)?;
    tmp.persist(out_path).map_err(|e| io_error(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn entry(slide: u32, start_sec: f64, end_sec: f64) -> SlideTimestampEntry {
        SlideTimestampEntry {
            slide,
            start_sec,
            end_sec,
        }
    }

    #[test]
    fn rounds_to_three_decimals() {
        assert_eq!(round_millis(34.830249), 34.83);
        assert_eq!(round_millis(36.8305), 36.831);
        assert_eq!(round_millis(184.3333564), 184.333);
        assert_eq!(round_millis(2.0), 2.0);
        assert!(round_millis(-0.0001).is_sign_positive());
    }

    #[test]
    fn formats_spec_schema() {
        let json = format_timestamps_json(&[
            entry(1, 0.0, 42.30049),
            entry(2, 42.30049, 88.1),
            entry(3, 88.1, 88.1),
        ])
        .unwrap();
        assert_eq!(
            json,
            r#"[
  {
    "slide": 1,
    "startSec": 0.0,
    "endSec": 42.3
  },
  {
    "slide": 2,
    "startSec": 42.3,
    "endSec": 88.1
  },
  {
    "slide": 3,
    "startSec": 88.1,
    "endSec": 88.1
  }
]
"#
        );
    }

    #[test]
    fn formats_empty_list() {
        assert_eq!(format_timestamps_json(&[]).unwrap(), "[]\n");
    }

    #[test]
    fn rejects_non_finite_seconds() {
        let err =
            format_timestamps_json(&[entry(1, 0.0, 1.0), entry(2, 1.0, f64::NAN)]).unwrap_err();
        assert_eq!(err, "スライド2の時刻が不正です（1〜NaN秒）");
    }

    #[test]
    fn existing_paths_keeps_order_and_drops_missing() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a_audio.m4a");
        let b = dir.path().join("a_slides.pdf");
        fs::write(&a, b"").unwrap();
        fs::write(&b, b"").unwrap();
        let missing = dir.path().join("a_timestamps.json");
        let paths: Vec<String> = [&b, &missing, &a]
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        assert_eq!(
            existing_paths(paths.clone()),
            vec![paths[0].clone(), paths[2].clone()]
        );
    }

    #[test]
    fn write_replaces_existing_file_without_leftovers() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("lecture_timestamps.json");
        fs::write(&out, "old").unwrap();
        write_atomically(&out, "[]\n").unwrap();
        assert_eq!(fs::read_to_string(&out).unwrap(), "[]\n");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn write_fails_for_missing_folder() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("none").join("x.json");
        let err = write_atomically(&out, "[]").unwrap_err();
        assert!(
            err.to_string().starts_with("ファイルを書き出せません（"),
            "{err}"
        );
    }

    #[test]
    fn command_writes_utf8_json_and_returns_path() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("講義 第1回_timestamps.json");
        let out_str = out.display().to_string();
        let returned = tauri::async_runtime::block_on(write_timestamps_json(
            out_str.clone(),
            vec![entry(1, 0.0, 1.23456)],
        ))
        .unwrap();
        assert_eq!(returned, out_str);
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(
            written,
            serde_json::json!([{ "slide": 1, "startSec": 0.0, "endSec": 1.235 }])
        );
    }
}
