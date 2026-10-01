use std::fmt;
use std::io;
use std::time::Duration;

/// ドメイン層の共通エラー。コマンド層で `to_string()` して日本語文言としてフロントへ返す。
#[derive(Debug)]
pub enum AppError {
    /// `AppState` に有効な設定がない
    SettingsNotLoaded,
    /// 外部プロセスを起動できない（実行ファイルがない、権限がない等）
    ProcessSpawn { program: String, source: io::Error },
    /// 外部プロセスが制限時間内に終わらず、強制終了した
    ProcessTimeout {
        program: String,
        timeout: Duration,
        stderr_tail: String,
    },
    /// 外部プロセスが非ゼロで終了した
    ProcessFailed {
        program: String,
        code: Option<i32>,
        stderr_tail: String,
    },
    /// ファイル操作等の失敗。`context` に何をしようとしたかを書く
    Io { context: String, source: io::Error },
    /// 上記に当てはまらない、文言だけのエラー
    Message(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::SettingsNotLoaded => write!(f, "設定が読み込まれていません"),
            AppError::ProcessSpawn { program, source } => {
                write!(f, "{program} を起動できません（{source}）")
            }
            AppError::ProcessTimeout {
                program,
                timeout,
                stderr_tail,
            } => {
                write!(
                    f,
                    "{program} が{}秒以内に終了しなかったため中断しました",
                    timeout.as_secs_f64()
                )?;
                write_stderr_tail(f, stderr_tail)
            }
            AppError::ProcessFailed {
                program,
                code,
                stderr_tail,
            } => {
                match code {
                    Some(code) => write!(f, "{program} が異常終了しました（終了コード {code}）")?,
                    None => write!(f, "{program} が異常終了しました（終了コードなし）")?,
                }
                write_stderr_tail(f, stderr_tail)
            }
            AppError::Io { context, source } => write!(f, "{context}（{source}）"),
            AppError::Message(message) => f.write_str(message),
        }
    }
}

fn write_stderr_tail(f: &mut fmt::Formatter<'_>, stderr_tail: &str) -> fmt::Result {
    if stderr_tail.is_empty() {
        Ok(())
    } else {
        write!(f, "\n{stderr_tail}")
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::ProcessSpawn { source, .. } | AppError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_message_includes_seconds_and_stderr_tail() {
        let err = AppError::ProcessTimeout {
            program: "ffmpeg.exe".into(),
            timeout: Duration::from_secs(300),
            stderr_tail: "last line".into(),
        };
        assert_eq!(
            err.to_string(),
            "ffmpeg.exe が300秒以内に終了しなかったため中断しました\nlast line"
        );
    }

    #[test]
    fn failed_message_omits_empty_stderr_tail() {
        let err = AppError::ProcessFailed {
            program: "soffice.exe".into(),
            code: Some(1),
            stderr_tail: String::new(),
        };
        assert_eq!(
            err.to_string(),
            "soffice.exe が異常終了しました（終了コード 1）"
        );
    }
}
