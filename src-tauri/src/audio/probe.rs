//! ffprobe で音声ファイルのコーデック・サンプルレート・チャンネル数・再生時間を取得する。

use std::path::Path;
use std::process::Command;

use serde_json::Value;

use super::PROCESS_TIMEOUT;
use crate::error::AppError;
use crate::process::run_with_timeout;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub duration_sec: f64,
}

/// `file` の最初の音声ストリームの情報を取得する。
/// ffprobe が非ゼロで終了した場合は stderr の末尾を含む `ProcessFailed` を返す
pub fn probe(ffprobe: &Path, file: &Path) -> Result<AudioInfo, AppError> {
    let mut cmd = Command::new(ffprobe);
    cmd.args([
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "stream=codec_name,sample_rate,channels:format=duration",
        "-of",
        "json",
    ])
    .arg(file);
    let output = run_with_timeout(cmd, PROCESS_TIMEOUT)?.ensure_success(ffprobe)?;
    parse_probe_json(&output.stdout).map_err(|reason| {
        AppError::Message(format!(
            "音声情報を取得できません（{}）：{reason}",
            file.display()
        ))
    })
}

/// ffprobe の `-of json` 出力を解析する（純粋関数）。
/// 数値項目は ffprobe の出力どおり文字列（`"44100"`）でも数値でも受け付ける。
/// 問題があれば理由の文言を返す
pub fn parse_probe_json(json: &str) -> Result<AudioInfo, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("ffprobe の出力を解析できません（{e}）"))?;
    let stream = root
        .get("streams")
        .and_then(Value::as_array)
        .and_then(|streams| streams.first())
        .ok_or("音声ストリームがありません")?;

    let codec = stream
        .get("codec_name")
        .and_then(Value::as_str)
        .filter(|codec| !codec.is_empty())
        .ok_or("コーデックを取得できません")?
        .to_string();
    let sample_rate = stream
        .get("sample_rate")
        .and_then(as_number)
        .filter(|rate| rate.fract() == 0.0 && *rate >= 1.0 && *rate <= f64::from(u32::MAX))
        .ok_or("サンプルレートを取得できません")? as u32;
    let channels = stream
        .get("channels")
        .and_then(as_number)
        .filter(|ch| ch.fract() == 0.0 && *ch >= 1.0 && *ch <= f64::from(u32::MAX))
        .ok_or("チャンネル数を取得できません")? as u32;
    let duration_sec = root
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(as_number)
        .filter(|sec| *sec >= 0.0)
        .ok_or("再生時間を取得できません")?;

    Ok(AudioInfo {
        codec,
        sample_rate,
        channels,
        duration_sec,
    })
}

/// 有限の数値、または数値として読める文字列（`"N/A"` 等は `None`）
fn as_number(value: &Value) -> Option<f64> {
    let number = match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    number.is_finite().then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ffprobe 9.0.2 が samples/ のppsxの音声（media1.m4a）に対して出力した内容
    const REAL_OUTPUT: &str = r#"{
    "programs": [

    ],
    "stream_groups": [

    ],
    "streams": [
        {
            "codec_name": "aac",
            "sample_rate": "44100",
            "channels": 2
        }
    ],
    "format": {
        "duration": "34.830249"
    }
}"#;

    #[test]
    fn parses_real_ffprobe_output() {
        assert_eq!(
            parse_probe_json(REAL_OUTPUT).unwrap(),
            AudioInfo {
                codec: "aac".into(),
                sample_rate: 44100,
                channels: 2,
                duration_sec: 34.830249,
            }
        );
    }

    #[test]
    fn accepts_numbers_as_well_as_strings() {
        let json = r#"{"streams":[{"codec_name":"mp3","sample_rate":48000,"channels":"1"}],
                       "format":{"duration":3}}"#;
        assert_eq!(
            parse_probe_json(json).unwrap(),
            AudioInfo {
                codec: "mp3".into(),
                sample_rate: 48000,
                channels: 1,
                duration_sec: 3.0,
            }
        );
    }

    #[test]
    fn uses_first_stream_only() {
        let json = r#"{"streams":[
                         {"codec_name":"aac","sample_rate":"48000","channels":1},
                         {"codec_name":"mp3","sample_rate":"44100","channels":2}],
                       "format":{"duration":"1.5"}}"#;
        let info = parse_probe_json(json).unwrap();
        assert_eq!(info.codec, "aac");
        assert_eq!(info.sample_rate, 48000);
    }

    #[test]
    fn errors_without_audio_stream() {
        let json = r#"{"programs":[],"streams":[],"format":{"duration":"1.0"}}"#;
        assert_eq!(
            parse_probe_json(json).unwrap_err(),
            "音声ストリームがありません"
        );
        // 解析できない入力に対して ffprobe は `{}` だけを出力する
        assert_eq!(
            parse_probe_json("{\n\n}").unwrap_err(),
            "音声ストリームがありません"
        );
    }

    #[test]
    fn errors_on_missing_or_invalid_fields() {
        let cases = [
            (
                r#"{"streams":[{"sample_rate":"48000","channels":1}],"format":{"duration":"1"}}"#,
                "コーデックを取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"N/A","channels":1}],"format":{"duration":"1"}}"#,
                "サンプルレートを取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"0","channels":1}],"format":{"duration":"1"}}"#,
                "サンプルレートを取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"44100.5","channels":1}],"format":{"duration":"1"}}"#,
                "サンプルレートを取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"48000","channels":0}],"format":{"duration":"1"}}"#,
                "チャンネル数を取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"48000","channels":1}],"format":{}}"#,
                "再生時間を取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"48000","channels":1}]}"#,
                "再生時間を取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"48000","channels":1}],"format":{"duration":"N/A"}}"#,
                "再生時間を取得できません",
            ),
            (
                r#"{"streams":[{"codec_name":"aac","sample_rate":"48000","channels":1}],"format":{"duration":"-1"}}"#,
                "再生時間を取得できません",
            ),
        ];
        for (json, expected) in cases {
            assert_eq!(parse_probe_json(json).unwrap_err(), expected, "{json}");
        }
    }

    #[test]
    fn errors_on_invalid_json() {
        let err = parse_probe_json("not json").unwrap_err();
        assert!(err.starts_with("ffprobe の出力を解析できません（"), "{err}");
    }

    #[test]
    fn probe_reports_spawn_failure() {
        let err = probe(
            Path::new("this-ffprobe-does-not-exist.exe"),
            Path::new("a.m4a"),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::ProcessSpawn { .. }), "{err:?}");
    }
}
