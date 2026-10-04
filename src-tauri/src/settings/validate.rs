use std::io;
use std::path::Path;

use serde_json::{Map, Value};

use super::{AppSettings, SettingsStatus};

/// 設定ファイルを読み込んで解析・検証する。`ok` のときだけ `settings` を返す
pub fn load_and_validate(path: &Path) -> SettingsStatus {
    let settings_path = path.display().to_string();
    let result = read_settings_file(path)
        .and_then(|json| parse_settings(&json))
        .map_err(|e| vec![e])
        .and_then(|settings| {
            let errors = validate(&settings);
            if errors.is_empty() {
                Ok(settings)
            } else {
                Err(errors)
            }
        });

    match result {
        Ok(settings) => SettingsStatus {
            ok: true,
            settings_path,
            settings: Some(settings),
            errors: Vec::new(),
        },
        Err(errors) => SettingsStatus {
            ok: false,
            settings_path,
            settings: None,
            errors,
        },
    }
}

fn read_settings_file(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        // メモ帳等で保存したBOM付きUTF-8も受け付ける
        Ok(text) => Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(format!(
            "app.settings.json が見つかりません（{}）。app.settings.example.json をコピーして作成してください",
            path.display()
        )),
        Err(e) => Err(format!(
            "設定ファイルを読み込めません（{}：{e}）",
            path.display()
        )),
    }
}

/// JSON文字列を `AppSettings` にする（純粋関数）。
/// 項目の有無と型を確認し、問題があれば最初の1件を項目名入りの文言で返す
pub fn parse_settings(json: &str) -> Result<AppSettings, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| format!("設定ファイルをJSONとして解釈できません（{e}）"))?;
    let Value::Object(fields) = &value else {
        return Err("設定ファイルの内容は { } で囲んだオブジェクトにしてください".into());
    };

    for name in ["ffmpegPath", "ffprobePath", "sofficePath"] {
        check_path_field(fields, name)?;
    }
    check_optional_field(
        fields,
        "silentSlideHandling",
        |v| matches!(v.as_str(), Some("insert_silence" | "skip")),
        "\"insert_silence\" または \"skip\"",
    )?;
    check_optional_field(
        fields,
        "silentSlideDefaultSec",
        Value::is_number,
        "0以上の数値",
    )?;
    check_optional_field(
        fields,
        "audioReencodeOnMismatch",
        Value::is_boolean,
        "true または false",
    )?;

    serde_json::from_value(value)
        .map_err(|e| format!("設定ファイルの内容を確認してください（{e}）"))
}

fn check_path_field(fields: &Map<String, Value>, name: &str) -> Result<(), String> {
    match fields.get(name) {
        None => Err(format!("設定ファイルに {name} がありません")),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(()),
        Some(_) => Err(format!(
            "設定ファイルの {name} には実行ファイルのパスを文字列で指定してください"
        )),
    }
}

fn check_optional_field(
    fields: &Map<String, Value>,
    name: &str,
    is_valid: impl Fn(&Value) -> bool,
    expected: &str,
) -> Result<(), String> {
    match fields.get(name) {
        Some(v) if !is_valid(v) => Err(format!(
            "設定ファイルの {name} には {expected} を指定してください"
        )),
        _ => Ok(()),
    }
}

/// 値の範囲と実行ファイルの実在を確認し、問題をすべて返す
pub fn validate(settings: &AppSettings) -> Vec<String> {
    let mut errors = Vec::new();
    for (name, path) in [
        ("ffmpegPath", &settings.ffmpeg_path),
        ("ffprobePath", &settings.ffprobe_path),
        ("sofficePath", &settings.soffice_path),
    ] {
        if path.is_file() {
            continue;
        }
        let reason = if path.exists() {
            "はファイルではありません"
        } else {
            "が見つかりません"
        };
        errors.push(format!(
            "設定ファイルの {name} を確認してください（{} {reason}）",
            path.display()
        ));
    }
    let sec = settings.silent_slide_default_sec;
    if !(sec.is_finite() && sec >= 0.0) {
        errors.push(format!(
            "設定ファイルの silentSlideDefaultSec には 0以上の数値 を指定してください（{sec} が指定されています）"
        ));
    }
    errors
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::settings::SilentSlideHandling;

    /// 3つの実行ファイルの代わりに空ファイルを置いた一時フォルダ
    fn fake_tools() -> (TempDir, [PathBuf; 3]) {
        let dir = TempDir::new().unwrap();
        let paths = ["ffmpeg.exe", "ffprobe.exe", "soffice.exe"].map(|n| dir.path().join(n));
        for p in &paths {
            fs::write(p, b"").unwrap();
        }
        (dir, paths)
    }

    fn required_only(paths: &[PathBuf; 3]) -> Value {
        json!({
            "ffmpegPath": paths[0],
            "ffprobePath": paths[1],
            "sofficePath": paths[2],
        })
    }

    #[test]
    fn parses_all_fields() {
        let settings = parse_settings(
            &json!({
                "ffmpegPath": "C:\\ffmpeg\\bin\\ffmpeg.exe",
                "ffprobePath": "C:\\ffmpeg\\bin\\ffprobe.exe",
                "sofficePath": "C:\\LibreOffice\\program\\soffice.exe",
                "silentSlideHandling": "skip",
                "silentSlideDefaultSec": 1.5,
                "audioReencodeOnMismatch": false,
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            settings.ffmpeg_path,
            PathBuf::from("C:\\ffmpeg\\bin\\ffmpeg.exe")
        );
        assert_eq!(settings.silent_slide_handling, SilentSlideHandling::Skip);
        assert_eq!(settings.silent_slide_default_sec, 1.5);
        assert!(!settings.audio_reencode_on_mismatch);
    }

    #[test]
    fn omitted_optional_fields_use_defaults() {
        let settings = parse_settings(
            r#"{ "ffmpegPath": "a", "ffprobePath": "b", "sofficePath": "c", "silentSlideDefaultSec": 0 }"#,
        )
        .unwrap();
        assert_eq!(
            settings.silent_slide_handling,
            SilentSlideHandling::InsertSilence
        );
        assert_eq!(settings.silent_slide_default_sec, 0.0);
        assert!(settings.audio_reencode_on_mismatch);

        let settings =
            parse_settings(r#"{ "ffmpegPath": "a", "ffprobePath": "b", "sofficePath": "c" }"#)
                .unwrap();
        assert_eq!(settings.silent_slide_default_sec, 3.0);
    }

    #[test]
    fn rejects_broken_json_and_non_object() {
        let err = parse_settings("{ \"ffmpegPath\": ").unwrap_err();
        assert!(err.contains("JSONとして解釈できません"), "{err}");
        let err = parse_settings("[]").unwrap_err();
        assert!(err.contains("オブジェクト"), "{err}");
    }

    #[test]
    fn rejects_missing_or_invalid_paths() {
        let err = parse_settings(r#"{ "ffprobePath": "b", "sofficePath": "c" }"#).unwrap_err();
        assert_eq!(err, "設定ファイルに ffmpegPath がありません");
        let err = parse_settings(r#"{ "ffmpegPath": "a", "ffprobePath": 1, "sofficePath": "c" }"#)
            .unwrap_err();
        assert!(err.contains("ffprobePath"), "{err}");
        let err =
            parse_settings(r#"{ "ffmpegPath": "a", "ffprobePath": "b", "sofficePath": " " }"#)
                .unwrap_err();
        assert!(err.contains("sofficePath"), "{err}");
    }

    #[test]
    fn rejects_invalid_optional_values() {
        let base = r#""ffmpegPath": "a", "ffprobePath": "b", "sofficePath": "c""#;
        for (field, expected_name) in [
            (r#""silentSlideHandling": "always""#, "silentSlideHandling"),
            (r#""silentSlideHandling": null"#, "silentSlideHandling"),
            (r#""silentSlideDefaultSec": "3""#, "silentSlideDefaultSec"),
            (
                r#""audioReencodeOnMismatch": "yes""#,
                "audioReencodeOnMismatch",
            ),
        ] {
            let err = parse_settings(&format!("{{ {base}, {field} }}")).unwrap_err();
            assert!(err.contains(expected_name), "{field}: {err}");
        }
    }

    #[test]
    fn validate_accepts_existing_files() {
        let (_dir, paths) = fake_tools();
        let settings = parse_settings(&required_only(&paths).to_string()).unwrap();
        assert!(validate(&settings).is_empty());
    }

    #[test]
    fn validate_reports_every_missing_path() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("missing");
        let settings = parse_settings(
            &required_only(&[
                missing.join("ffmpeg.exe"),
                missing.join("ffprobe.exe"),
                dir.path().to_path_buf(), // フォルダはファイルとして扱わない
            ])
            .to_string(),
        )
        .unwrap();

        let errors = validate(&settings);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert_eq!(
            errors[0],
            format!(
                "設定ファイルの ffmpegPath を確認してください（{} が見つかりません）",
                missing.join("ffmpeg.exe").display()
            )
        );
        assert!(errors[1].contains("ffprobePath"));
        assert!(errors[2].contains("sofficePath") && errors[2].contains("ファイルではありません"));
    }

    #[test]
    fn validate_rejects_negative_default_sec() {
        let (_dir, paths) = fake_tools();
        let mut value = required_only(&paths);
        value["silentSlideDefaultSec"] = json!(-1);
        let settings = parse_settings(&value.to_string()).unwrap();
        let errors = validate(&settings);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("silentSlideDefaultSec"));
    }

    #[test]
    fn load_reports_missing_file_with_searched_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("app.settings.json");
        let status = load_and_validate(&path);
        assert!(!status.ok);
        assert!(status.settings.is_none());
        assert_eq!(status.settings_path, path.display().to_string());
        assert_eq!(status.errors.len(), 1);
        assert!(status.errors[0].contains(&path.display().to_string()));
        assert!(status.errors[0].contains("app.settings.example.json"));
    }

    #[test]
    fn load_accepts_valid_file_with_bom() {
        let (dir, paths) = fake_tools();
        let path = dir.path().join("app.settings.json");
        fs::write(&path, format!("\u{feff}{}", required_only(&paths))).unwrap();
        let status = load_and_validate(&path);
        assert!(status.ok, "{:?}", status.errors);
        assert_eq!(status.settings.unwrap().soffice_path, paths[2]);
    }

    #[test]
    fn load_returns_no_settings_when_validation_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("app.settings.json");
        fs::write(
            &path,
            r#"{ "ffmpegPath": "x", "ffprobePath": "y", "sofficePath": "z" }"#,
        )
        .unwrap();
        let status = load_and_validate(&path);
        assert!(!status.ok);
        assert!(status.settings.is_none());
        assert_eq!(status.errors.len(), 3);
    }
}
