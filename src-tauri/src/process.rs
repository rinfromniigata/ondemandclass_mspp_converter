//! 外部プロセス（ffmpeg / ffprobe / soffice）の実行。
//! タイムアウト制御、コンソール窓の非表示、stdout/stderr の別スレッド読み取りを行う。

use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::AppError;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// プロセス終了後、パイプを読み切るまで待つ上限。
/// 孫プロセスがパイプを引き継いで生き残った場合（sofficeのランチャー等）に、
/// 読み取りスレッドの完了を待ち続けて止まらないようにする
const PIPE_DRAIN_GRACE: Duration = Duration::from_secs(5);
/// エラー文言に含める stderr の行数
const STDERR_TAIL_LINES: usize = 20;

#[derive(Debug)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessOutput {
    /// 非ゼロ終了なら stderr の末尾を含む `ProcessFailed` にする
    pub fn ensure_success(self, program: &Path) -> Result<ProcessOutput, AppError> {
        if self.status.success() {
            Ok(self)
        } else {
            Err(AppError::ProcessFailed {
                program: program.display().to_string(),
                code: self.status.code(),
                stderr_tail: tail_lines(&self.stderr, STDERR_TAIL_LINES),
            })
        }
    }
}

/// `cmd` を実行し、終了を待って出力を返す。非ゼロ終了はエラーにしない（`ensure_success` で判定する）。
/// stdin・stdout・stderr の設定はこの関数で上書きする。
pub fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<ProcessOutput, AppError> {
    let program = Path::new(cmd.get_program()).display().to_string();
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut cmd);

    let mut child = cmd.spawn().map_err(|source| AppError::ProcessSpawn {
        program: program.clone(),
        source,
    })?;
    let stdout_rx = spawn_reader(child.stdout.take());
    let stderr_rx = spawn_reader(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    break None;
                }
                thread::sleep(POLL_INTERVAL.min(deadline - now));
            }
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::Io {
                    context: format!("{program} の終了を待てませんでした"),
                    source,
                });
            }
        }
    };

    let Some(status) = status else {
        // kill は終了済みの場合にもエラーを返すだけなので結果は見ない
        let _ = child.kill();
        let _ = child.wait();
        let drain_deadline = Instant::now() + PIPE_DRAIN_GRACE;
        let stderr = receive_until(&stderr_rx, drain_deadline);
        return Err(AppError::ProcessTimeout {
            program,
            timeout,
            stderr_tail: tail_lines(&stderr, STDERR_TAIL_LINES),
        });
    };

    let drain_deadline = Instant::now() + PIPE_DRAIN_GRACE;
    let stdout = receive_until(&stdout_rx, drain_deadline);
    let stderr = receive_until(&stderr_rx, drain_deadline);
    Ok(ProcessOutput {
        status,
        stdout,
        stderr,
    })
}

#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_cmd: &mut Command) {}

/// パイプを別スレッドで最後まで読み、読み終えたら文字列を送る
fn spawn_reader<R: Read + Send + 'static>(pipe: Option<R>) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    if let Some(mut pipe) = pipe {
        thread::spawn(move || {
            let mut buf = Vec::new();
            // 読み取りエラー時もそこまでに読めた分を返す
            let _ = pipe.read_to_end(&mut buf);
            let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
        });
    }
    rx
}

/// 期限までに読み取りスレッドが終わらなければ空文字列とする
fn receive_until(rx: &Receiver<String>, deadline: Instant) -> String {
    rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_default()
}

/// 末尾 `n` 行（末尾の空行は除く）
fn tail_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OS標準のシェルで `script` を実行する Command
    fn shell(script: &str) -> Command {
        if cfg!(windows) {
            let mut cmd = Command::new("cmd");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg("/D /C").raw_arg(script);
            }
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(script);
            cmd
        }
    }

    /// シェルを介さず直接長時間待機するプロセス（kill で確実に止められるようにする）
    fn long_running() -> Command {
        if cfg!(windows) {
            let mut cmd = Command::new("ping");
            cmd.args(["-n", "30", "127.0.0.1"]);
            cmd
        } else {
            let mut cmd = Command::new("sleep");
            cmd.arg("30");
            cmd
        }
    }

    #[test]
    fn captures_stdout_and_stderr_on_success() {
        let output = run_with_timeout(
            shell("echo out-line&& echo err-line 1>&2"),
            Duration::from_secs(30),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.trim(), "out-line");
        assert_eq!(output.stderr.trim(), "err-line");
        assert!(output.ensure_success(Path::new("cmd")).is_ok());
    }

    #[test]
    fn nonzero_exit_is_returned_and_ensure_success_fails() {
        let output =
            run_with_timeout(shell("echo boom 1>&2&& exit 3"), Duration::from_secs(30)).unwrap();
        assert_eq!(output.status.code(), Some(3));

        let err = output.ensure_success(Path::new("tool.exe")).unwrap_err();
        match &err {
            AppError::ProcessFailed {
                program,
                code,
                stderr_tail,
            } => {
                assert_eq!(program, "tool.exe");
                assert_eq!(*code, Some(3));
                assert_eq!(stderr_tail.trim(), "boom");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(err.to_string().contains("終了コード 3"));
    }

    #[test]
    fn kills_process_on_timeout() {
        let started = Instant::now();
        let err = run_with_timeout(long_running(), Duration::from_millis(500)).unwrap_err();
        assert!(
            matches!(err, AppError::ProcessTimeout { timeout, .. } if timeout == Duration::from_millis(500)),
            "unexpected error: {err:?}"
        );
        // 30秒待たずに戻ること（kill されている）
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn spawn_failure_names_the_program() {
        let missing = std::env::temp_dir()
            .join("no_such_dir_for_test")
            .join("missing.exe");
        let err = run_with_timeout(Command::new(&missing), Duration::from_secs(5)).unwrap_err();
        assert!(
            matches!(err, AppError::ProcessSpawn { .. }),
            "unexpected error: {err:?}"
        );
        let message = err.to_string();
        assert!(message.starts_with(&missing.display().to_string()));
        assert!(message.contains("を起動できません"));
    }

    #[test]
    fn large_output_does_not_block() {
        // パイプのバッファ（Windowsでは数KB）を大きく超える出力でも詰まらないこと
        let script = if cfg!(windows) {
            "for /L %i in (1,1,20000) do @echo line%i"
        } else {
            "i=1; while [ $i -le 20000 ]; do echo line$i; i=$((i+1)); done"
        };
        let output = run_with_timeout(shell(script), Duration::from_secs(60)).unwrap();
        assert!(output.status.success());
        let lines: Vec<&str> = output.stdout.lines().collect();
        assert_eq!(lines.len(), 20000);
        assert_eq!(lines.last().map(|l| l.trim()), Some("line20000"));
    }

    #[test]
    fn tail_lines_keeps_last_n_lines() {
        assert_eq!(tail_lines("a\nb\nc\nd\n\n", 2), "c\nd");
        assert_eq!(tail_lines("a\r\nb\r\n", 5), "a\nb");
        assert_eq!(tail_lines("", 20), "");
    }
}
