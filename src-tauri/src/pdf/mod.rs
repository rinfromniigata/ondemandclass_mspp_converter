//! LibreOffice（soffice）による pptx/ppsx の PDF 変換。

pub mod strip_audio;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use tempfile::TempDir;

use self::strip_audio::write_pdf_source;
use crate::error::AppError;
use crate::fs_util::move_or_copy;
use crate::process::run_with_timeout;

/// soffice 1回あたりのタイムアウト
pub const SOFFICE_TIMEOUT: Duration = Duration::from_secs(120);

/// `input` を PDF に変換して `out_path` に書き出し、`out_path` を返す。
///
/// - `profile_dir` はアプリ専用の LibreOffice ユーザープロファイル（起動をまたいで再利用する）。
///   ユーザーが起動中の LibreOffice とプロファイルを共有すると、変換がそちらへ渡されて失敗するため
/// - 出力は一時フォルダに作ってから `out_path` へ移す（入力と同じフォルダの `<basename>.pdf` を上書きしない）
/// - soffice には、音声の図形（再生アイコン）を除去した一時コピーを渡す（`strip_audio`）。
///   コピーは入力と同じファイル名にし、出力名 `<stem>.pdf` と拡張子による pptx/ppsx の判別を変えない
pub fn convert_to_pdf(
    soffice: &Path,
    profile_dir: &Path,
    input: &Path,
    out_path: &Path,
) -> Result<PathBuf, AppError> {
    let invalid_name =
        || AppError::Message(format!("入力ファイル名が不正です（{}）", input.display()));
    let stem = input
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(invalid_name)?;
    let file_name = input.file_name().ok_or_else(invalid_name)?;
    fs::create_dir_all(profile_dir).map_err(|source| AppError::Io {
        context: format!(
            "LibreOffice のプロファイルフォルダを作成できません（{}）",
            profile_dir.display()
        ),
        source,
    })?;
    let tmp = TempDir::new().map_err(|source| AppError::Io {
        context: "一時フォルダを作成できません".into(),
        source,
    })?;

    // 出力先（tmp 直下）とは別のサブフォルダに、加工済みコピーを置く
    let source_dir = tmp.path().join("src");
    fs::create_dir(&source_dir).map_err(|source| AppError::Io {
        context: format!("一時フォルダを作成できません（{}）", source_dir.display()),
        source,
    })?;
    let source = source_dir.join(file_name);
    write_pdf_source(input, &source)?;

    let mut cmd = Command::new(soffice);
    cmd.arg(format!("-env:UserInstallation={}", file_url(profile_dir)))
        .args([
            "--headless",
            "--norestore",
            "--convert-to",
            "pdf",
            "--outdir",
        ])
        .arg(tmp.path())
        .arg(&source);
    let output = run_with_timeout(cmd, SOFFICE_TIMEOUT)?.ensure_success(soffice)?;

    let mut pdf_name = stem.to_os_string();
    pdf_name.push(".pdf");
    let pdf = tmp.path().join(pdf_name);
    if !pdf.is_file() {
        let mut message = format!("PDFが生成されませんでした（{}）", input.display());
        let tail = output.output_tail();
        if !tail.is_empty() {
            message.push('\n');
            message.push_str(&tail);
        }
        return Err(AppError::Message(message));
    }
    move_or_copy(&pdf, out_path).map_err(|source| AppError::Io {
        context: format!("PDFを保存できません（{}）", out_path.display()),
        source,
    })?;
    Ok(out_path.to_path_buf())
}

/// 絶対パスを `file:` URL にする（純粋関数）。`\` は `/` にし、英数字と `-._~/:` 以外は
/// UTF-8 のバイト単位でパーセントエンコードする
///
/// - `C:\Users\a b` → `file:///C:/Users/a%20b`
/// - `\\server\share\x` → `file://server/share/x`
/// - `\\?\C:\x`（拡張パス）→ `file:///C:/x`
/// - `/home/a` → `file:///home/a`
pub fn file_url(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let normalized = if let Some(unc) = raw.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else if let Some(local) = raw.strip_prefix("//?/") {
        local.to_owned()
    } else {
        raw
    };
    let (prefix, rest) = match normalized.strip_prefix("//") {
        Some(unc) => ("file://", unc),
        None if normalized.starts_with('/') => ("file://", normalized.as_str()),
        None => ("file:///", normalized.as_str()),
    };

    let mut url = String::from(prefix);
    for byte in rest.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/:".contains(&byte) {
            url.push(char::from(byte));
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_for_windows_drive_path() {
        assert_eq!(
            file_url(Path::new(
                r"C:\Users\ofuchirin\AppData\Local\app\lo_profile"
            )),
            "file:///C:/Users/ofuchirin/AppData/Local/app/lo_profile"
        );
    }

    #[test]
    fn file_url_encodes_spaces_and_non_ascii() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\山田 花子\p#1%")),
            "file:///C:/Users/%E5%B1%B1%E7%94%B0%20%E8%8A%B1%E5%AD%90/p%231%25"
        );
    }

    #[test]
    fn file_url_for_unc_and_extended_paths() {
        assert_eq!(
            file_url(Path::new(r"\\server\share\profile")),
            "file://server/share/profile"
        );
        assert_eq!(file_url(Path::new(r"\\?\C:\x\y")), "file:///C:/x/y");
        assert_eq!(
            file_url(Path::new(r"\\?\UNC\server\share\p")),
            "file://server/share/p"
        );
    }

    #[test]
    fn file_url_for_unix_path() {
        assert_eq!(
            file_url(Path::new("/home/user/.local/share/app/lo_profile")),
            "file:///home/user/.local/share/app/lo_profile"
        );
    }

    #[test]
    fn convert_reports_spawn_failure() {
        let dir = TempDir::new().unwrap();
        // soffice の前に加工済みコピーを作るため、入力は読める zip にする
        let input = dir.path().join("lecture.ppsx");
        write_minimal_zip(&input);
        let err = convert_to_pdf(
            Path::new("this-soffice-does-not-exist.exe"),
            &dir.path().join("profile"),
            &input,
            &dir.path().join("lecture_slides.pdf"),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::ProcessSpawn { .. }), "{err:?}");
        // プロファイルフォルダは起動前に作る
        assert!(dir.path().join("profile").is_dir());
    }

    #[test]
    fn convert_fails_before_soffice_when_copy_cannot_be_made() {
        // 加工済みコピーを作れなければ、soffice を起動せずにエラーにする（再生アイコンつきのPDFを出さない）
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("lecture.pptx");
        fs::write(&input, b"not a zip").unwrap();
        let out = dir.path().join("lecture_slides.pdf");
        let err = convert_to_pdf(
            Path::new("this-soffice-does-not-exist.exe"),
            &dir.path().join("profile"),
            &input,
            &out,
        )
        .unwrap_err();
        assert!(
            err.to_string().starts_with("pptx/ppsxとして読み込めません"),
            "{err}"
        );
        assert!(!out.exists());
    }

    fn write_minimal_zip(path: &Path) {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        zip.start_file(
            "ppt/slides/slide1.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"<p:sld/>").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn convert_rejects_input_without_file_name() {
        let dir = TempDir::new().unwrap();
        let err = convert_to_pdf(
            Path::new("soffice.exe"),
            &dir.path().join("profile"),
            Path::new(""),
            &dir.path().join("out.pdf"),
        )
        .unwrap_err();
        assert!(
            err.to_string().starts_with("入力ファイル名が不正です"),
            "{err}"
        );
    }
}
