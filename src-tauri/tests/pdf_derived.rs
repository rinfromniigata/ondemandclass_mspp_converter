//! 派生サンプル（imple 6.4）の `original` を実際の soffice で PDF にし、
//! 音声の再生アイコン・音声データがPDFに出ないことを確かめる（T9-5）。
//! 実ファイルは Git で除外しているため `#[ignore]`。`bun scripts/make_samples.ts` の後に、
//! 環境変数 `SOFFICE_PATH` を設定して `cargo test --test pdf_derived -- --ignored` で実行する。
//! soffice はエージェントのサンドボックス内では異常終了するため、サンドボックス外で実行する

use std::fs;
use std::path::{Path, PathBuf};

use ondemandclass_mspp_converter_lib::pdf::convert_to_pdf;
use ondemandclass_mspp_converter_lib::pdf::strip_audio::write_pdf_source;
use ondemandclass_mspp_converter_lib::pptx::extract_from_file;
use tempfile::TempDir;

const FORMATS: [&str; 2] = ["pptx", "ppsx"];

fn sample(kind: &str, ext: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples/derived")
        .join(ext)
        .join(format!("{kind}.{ext}"))
}

fn soffice() -> PathBuf {
    PathBuf::from(
        std::env::var("SOFFICE_PATH")
            .expect("環境変数 SOFFICE_PATH に実行ファイルのパスを設定してください"),
    )
}

/// `needle` のバイト列としての出現数
fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

/// ページ数。LibreOffice は各ページを `/Type/Page` と書く（`/Type/Pages` はページツリーなので除く）
fn page_count(pdf: &[u8]) -> usize {
    let needle = b"/Type/Page";
    pdf.windows(needle.len() + 1)
        .filter(|window| window.starts_with(needle) && window[needle.len()] != b's')
        .count()
}

#[test]
#[ignore = "soffice が必要（SOFFICE_PATH）・サンドボックス外で実行"]
fn original_pdf_has_no_audio_icons_or_embedded_audio() {
    let soffice = soffice();
    let dir = TempDir::new().unwrap();
    let profile = dir.path().join("lo_profile");
    for ext in FORMATS {
        let input = sample("original", ext);
        let before = fs::read(&input).unwrap_or_else(|e| panic!("{}: {e}", input.display()));
        let map = extract_from_file(&input).unwrap();
        let audio_shapes: usize = map.slides.iter().map(|s| s.audio_media_paths.len()).sum();
        assert!(audio_shapes > 0, "{ext}: 原本に音声がある前提");

        // コピーでは原本の音声図形がすべて除去される
        let copy = dir.path().join(format!("copy.{ext}"));
        assert_eq!(
            write_pdf_source(&input, &copy).unwrap(),
            audio_shapes,
            "{ext}"
        );

        let out = dir.path().join(format!("original_{ext}_slides.pdf"));
        let written = convert_to_pdf(&soffice, &profile, &input, &out)
            .unwrap_or_else(|e| panic!("{ext}: {e}"));
        assert_eq!(written, out);

        let pdf = fs::read(&out).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "{ext}");
        assert_eq!(count(&pdf, b"/Subtype/Screen"), 0, "{ext}: 再生用の注釈");
        assert_eq!(count(&pdf, b"/EmbeddedFile"), 0, "{ext}: 埋め込み音声");
        assert_eq!(page_count(&pdf), map.slides.len(), "{ext}: ページ数");

        // 入力は変更されない
        assert_eq!(fs::read(&input).unwrap(), before, "{ext}");
        eprintln!(
            "{ext}: {}ページ・音声図形{audio_shapes}個を除去・PDF {}バイト",
            map.slides.len(),
            pdf.len()
        );
    }
}

#[test]
fn page_count_ignores_page_tree() {
    let pdf = b"<</Type/Pages/Count 2>> <</Type/Page/Parent 1 0 R>> <</Type/Page>>";
    assert_eq!(page_count(pdf), 2);
    assert_eq!(count(pdf, b"/Type/Page"), 3);
}
