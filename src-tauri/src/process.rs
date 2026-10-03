//! 外部プロセス（ffmpeg / ffprobe / soffice）の実行。
//! タイムアウト制御、コンソール窓の非表示、stdout/stderr の別スレッド読み取りを行う。

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
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

    /// stdout と stderr の末尾（空でないものを改行でつなぐ）。正常終了したのに成果物がない場合の文言に使う
    pub fn output_tail(&self) -> String {
        [&self.stdout, &self.stderr]
            .into_iter()
            .map(|text| tail_lines(text, STDERR_TAIL_LINES))
            .filter(|tail| !tail.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// `cmd` を実行し、終了を待って出力を返す。非ゼロ終了はエラーにしない（`ensure_success` で判定する）。
/// stdin・stdout・stderr の設定はこの関数で上書きする。
pub fn run_with_timeout(cmd: Command, timeout: Duration) -> Result<ProcessOutput, AppError> {
    run_with_timeout_streaming(cmd, timeout, &mut |_| {})
}

/// `run_with_timeout` と同じく実行し、stdout を1行読むごとに `on_stdout_line` へ渡す（行末の改行は除く）。
/// コールバックは呼び出し元のスレッドで、出力された順に呼ぶ。渡した行は `ProcessOutput.stdout` にも残す。
/// タイムアウトした場合、それまでに読めた行は渡すが、終了後の残りは渡さない
pub fn run_with_timeout_streaming(
    mut cmd: Command,
    timeout: Duration,
    on_stdout_line: &mut dyn FnMut(&str),
) -> Result<ProcessOutput, AppError> {
    let program = Path::new(cmd.get_program()).display().to_string();
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut cmd);

    let mut child = cmd.spawn().map_err(|source| AppError::ProcessSpawn {
        program: program.clone(),
        source,
    })?;
    let mut stdout = LineSink::new(spawn_line_reader(child.stdout.take()), on_stdout_line);
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
                // 待ち時間の間も stdout の行を受け取って渡す
                stdout.pump(POLL_INTERVAL.min(deadline - now));
            }
            Err(source) => {
                kill_tree(&mut child);
                let _ = child.wait();
                return Err(AppError::Io {
                    context: format!("{program} の終了を待てませんでした"),
                    source,
                });
            }
        }
    };

    let Some(status) = status else {
        kill_tree(&mut child);
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
    stdout.drain_until(drain_deadline);
    let stderr = receive_until(&stderr_rx, drain_deadline);
    Ok(ProcessOutput {
        status,
        stdout: stdout.into_text(),
        stderr,
    })
}

/// stdout の行を受け取り、コールバックへ渡しながら全体の文字列にためる
struct LineSink<'a> {
    rx: Receiver<String>,
    on_line: &'a mut dyn FnMut(&str),
    text: String,
    /// 読み取りスレッドが終わった（EOF・読み取りエラー）か
    closed: bool,
}

impl<'a> LineSink<'a> {
    fn new(rx: Receiver<String>, on_line: &'a mut dyn FnMut(&str)) -> Self {
        Self {
            rx,
            on_line,
            text: String::new(),
            closed: false,
        }
    }

    /// 最大 `wait` だけ次の行を待ち、届いていた行をすべて渡す。読み取りが終わっていれば `wait` だけ眠る
    fn pump(&mut self, wait: Duration) {
        if self.closed {
            thread::sleep(wait);
            return;
        }
        match self.rx.recv_timeout(wait) {
            Ok(line) => self.deliver(line),
            Err(RecvTimeoutError::Timeout) => return,
            Err(RecvTimeoutError::Disconnected) => {
                self.closed = true;
                return;
            }
        }
        loop {
            match self.rx.try_recv() {
                Ok(line) => self.deliver(line),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    break;
                }
            }
        }
    }

    /// プロセス終了後、読み取りが終わるか期限が来るまで残りの行を渡す
    fn drain_until(&mut self, deadline: Instant) {
        while !self.closed {
            match self
                .rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(line) => self.deliver(line),
                Err(RecvTimeoutError::Disconnected) => self.closed = true,
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
    }

    fn deliver(&mut self, line: String) {
        (self.on_line)(trim_line_end(&line));
        self.text.push_str(&line);
    }

    fn into_text(self) -> String {
        self.text
    }
}

#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_cmd: &mut Command) {}

/// 子プロセスを子孫ごと強制終了する。
/// Windows では `soffice.exe` が実体の `soffice.bin` を子プロセスとして起動するため、
/// 直接の子だけを kill すると `soffice.bin` が残り、プロファイルを握ったままになる。
/// `taskkill /T /F` で子孫ごと終了させ、失敗した場合に備えて直接の子も kill する
#[cfg(windows)]
fn kill_tree(child: &mut Child) {
    let mut taskkill = Command::new("taskkill");
    taskkill
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut taskkill);
    let _ = taskkill.status();
    // kill は終了済みの場合にもエラーを返すだけなので結果は見ない
    let _ = child.kill();
}

/// Windows 以外は直接の子だけを終了する（子孫は残りうる）
#[cfg(not(windows))]
fn kill_tree(child: &mut Child) {
    let _ = child.kill();
}

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

/// パイプを別スレッドで1行ずつ読み、行末の改行を含めたまま1行ずつ送る。
/// 最後の行が改行で終わらない場合もそのまま送る。読み終えるとスレッドが終わり、受信側は切断を受け取る
fn spawn_line_reader<R: Read + Send + 'static>(pipe: Option<R>) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    if let Some(pipe) = pipe {
        thread::spawn(move || {
            let mut reader = BufReader::new(pipe);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                // 読み取りエラー時はそこまでに送った分で終える
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        // 改行は ASCII のため、行単位で変換しても複数バイト文字は分断されない
                        if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }
    rx
}

/// 行末の `\n` / `\r\n` を除く
fn trim_line_end(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
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

    /// 孫プロセスも終了させる。孫がパイプを引き継いで残ると、パイプを読み切る待ち
    /// （PIPE_DRAIN_GRACE = 5秒）まで戻らないため、戻るまでの時間で確かめる
    #[cfg(windows)]
    #[test]
    fn kills_grandchildren_on_timeout() {
        let started = Instant::now();
        let err = run_with_timeout(
            shell("ping -n 30 127.0.0.1 > nul"),
            Duration::from_millis(500),
        )
        .unwrap_err();
        assert!(
            matches!(err, AppError::ProcessTimeout { .. }),
            "unexpected error: {err:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "elapsed: {:?}",
            started.elapsed()
        );
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
    fn streaming_passes_lines_in_order_without_line_endings() {
        let mut lines = Vec::new();
        let output = run_with_timeout_streaming(
            shell("echo alpha&& echo beta&& echo gamma"),
            Duration::from_secs(30),
            &mut |line| lines.push(line.to_string()),
        )
        .unwrap();
        assert!(output.status.success());
        // Windows の cmd は \r\n で出力するが、渡す行には改行を含めない
        assert_eq!(lines, ["alpha", "beta", "gamma"]);
        // stdout には元の出力（改行込み）を残す
        assert_eq!(
            output.stdout.lines().collect::<Vec<_>>(),
            ["alpha", "beta", "gamma"]
        );
        assert!(output.stdout.ends_with('\n'));
    }

    #[test]
    fn streaming_passes_lines_before_process_exits() {
        // 1行目を出してから約2秒待って終わる
        let script = if cfg!(windows) {
            "echo first&& ping -n 3 127.0.0.1 > nul&& echo second"
        } else {
            "echo first; sleep 2; echo second"
        };
        let mut first_at = None;
        let mut lines = Vec::new();
        let output =
            run_with_timeout_streaming(shell(script), Duration::from_secs(30), &mut |line| {
                if line == "first" {
                    first_at = Some(Instant::now());
                }
                lines.push(line.to_string());
            })
            .unwrap();
        let returned_at = Instant::now();
        assert!(output.status.success());
        assert_eq!(lines, ["first", "second"]);
        let first_at = first_at.expect("first が渡されていない");
        assert!(
            returned_at - first_at >= Duration::from_secs(1),
            "終了の {:?} 前に渡された（終了前に渡されていない）",
            returned_at - first_at
        );
    }

    #[test]
    fn streaming_passes_last_line_without_newline() {
        let script = if cfg!(windows) {
            "echo one&& <nul set /p =tail"
        } else {
            "echo one; printf tail"
        };
        let mut lines = Vec::new();
        let output =
            run_with_timeout_streaming(shell(script), Duration::from_secs(30), &mut |line| {
                lines.push(line.to_string())
            })
            .unwrap();
        // cmd の `set /p` は入力が空だと終了コード1になるため、終了コードは確かめない
        assert_eq!(lines, ["one", "tail"]);
        assert!(output.stdout.ends_with("tail"));
    }

    #[test]
    fn streaming_kills_process_on_timeout() {
        // 1行目を出したあと長く待つ。タイムアウトまでに出た行は渡される
        let script = if cfg!(windows) {
            "echo started&& ping -n 30 127.0.0.1 > nul"
        } else {
            "echo started; sleep 30"
        };
        let started = Instant::now();
        let mut lines = Vec::new();
        let err =
            run_with_timeout_streaming(shell(script), Duration::from_millis(1500), &mut |line| {
                lines.push(line.to_string())
            })
            .unwrap_err();
        assert!(
            matches!(err, AppError::ProcessTimeout { timeout, .. } if timeout == Duration::from_millis(1500)),
            "unexpected error: {err:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(lines, ["started"]);
    }

    #[test]
    fn trim_line_end_removes_lf_and_crlf_only() {
        assert_eq!(trim_line_end("a\n"), "a");
        assert_eq!(trim_line_end("a\r\n"), "a");
        assert_eq!(trim_line_end("a"), "a");
        assert_eq!(trim_line_end("a \r"), "a ");
        assert_eq!(trim_line_end("\n"), "");
        assert_eq!(trim_line_end("a\n\n"), "a\n");
    }

    #[test]
    fn tail_lines_keeps_last_n_lines() {
        assert_eq!(tail_lines("a\nb\nc\nd\n\n", 2), "c\nd");
        assert_eq!(tail_lines("a\r\nb\r\n", 5), "a\nb");
        assert_eq!(tail_lines("", 20), "");
    }
}
