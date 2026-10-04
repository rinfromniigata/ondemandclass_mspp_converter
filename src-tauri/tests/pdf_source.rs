//! PDF変換用コピー（`write_pdf_source`）の結合テスト（imple 6.1）。
//! `PptxBuilder` で組み立てたファイルを書き直し、音声図形だけが除かれることを確かめる

mod common;

use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use common::fixture::{PackageKind, PptxBuilder, SlideBuilder};
use ondemandclass_mspp_converter_lib::pdf::strip_audio::write_pdf_source;
use ondemandclass_mspp_converter_lib::pptx::extract_from_file;
use ondemandclass_mspp_converter_lib::pptx::slide::{parse_slide, MediaKind};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// 音声つき2枚・音声なし1枚・動画つき1枚
fn lecture(kind: PackageKind) -> PptxBuilder {
    PptxBuilder::new()
        .kind(kind)
        .slide(
            1,
            SlideBuilder::new()
                .text(3)
                .audio(4, "rId2")
                .timing_audio(&[4])
                .advance_ms_alternate(5000, 9000)
                .audio_rel("rId2", "../media/media1.m4a"),
        )
        .slide(2, SlideBuilder::new().text(3))
        .slide(
            3,
            SlideBuilder::new()
                .audio(4, "rId2")
                .text(5)
                .audio(6, "rId3")
                .timing_audio(&[6, 4])
                .audio_rel("rId2", "../media/media2.m4a")
                .audio_rel("rId3", "../media/media3.m4a"),
        )
        .slide(
            4,
            SlideBuilder::new()
                .video(7, "rId2")
                .video_rel("rId2", "../media/video1.mp4"),
        )
        .media("media1.m4a", b"audio-1")
        .media("media2.m4a", b"audio-2")
        .media("media3.m4a", b"audio-3")
        .media("video1.mp4", b"video-1")
}

fn open(path: &Path) -> ZipArchive<File> {
    ZipArchive::new(File::open(path).unwrap()).unwrap()
}

fn read_entry(zip: &mut ZipArchive<File>, name: &str) -> String {
    let mut text = String::new();
    zip.by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

/// 圧縮されたままのバイト列
fn raw_bytes(zip: &mut ZipArchive<File>, index: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    zip.by_index_raw(index)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

/// エントリ名をインデックス順（zip内の並び）に返す
fn names_in_order(zip: &ZipArchive<File>) -> Vec<String> {
    (0..zip.len())
        .map(|i| zip.name_for_index(i).unwrap().to_owned())
        .collect()
}

#[test]
fn removes_audio_shapes_from_slides() {
    for kind in [PackageKind::Pptx, PackageKind::Ppsx] {
        let dir = TempDir::new().unwrap();
        let input = lecture(kind).write_to(dir.path(), "lecture");
        let dest = dir.path().join("copy.zip");

        assert_eq!(write_pdf_source(&input, &dest).unwrap(), 3);

        let mut out = open(&dest);
        for slide in 1..=3 {
            let xml = read_entry(&mut out, &format!("ppt/slides/slide{slide}.xml"));
            let parts = parse_slide(&xml).unwrap();
            assert!(parts.media_shapes.is_empty(), "slide{slide}: {xml}");
        }
        // 音声以外の図形・画面切り替え・timing は残る
        let slide1 = read_entry(&mut out, "ppt/slides/slide1.xml");
        assert!(slide1.contains(r#"name="テキスト 3""#));
        assert_eq!(parse_slide(&slide1).unwrap().advance_sec, Some(5.0));
        assert_eq!(parse_slide(&slide1).unwrap().timing_audio_order, vec!["4"]);
        let slide3 = read_entry(&mut out, "ppt/slides/slide3.xml");
        assert!(slide3.contains(r#"name="テキスト 5""#));
        // 動画は残る
        let slide4 = parse_slide(&read_entry(&mut out, "ppt/slides/slide4.xml")).unwrap();
        assert_eq!(slide4.media_shapes.len(), 1);
        assert_eq!(slide4.media_shapes[0].kind, MediaKind::Video);
    }
}

#[test]
fn copy_is_still_a_readable_package() {
    for kind in [PackageKind::Pptx, PackageKind::Ppsx] {
        let dir = TempDir::new().unwrap();
        let input = lecture(kind).write_to(dir.path(), "lecture");
        let ext = input.extension().unwrap().to_owned();
        let dest = dir.path().join("src").join("lecture").with_extension(ext);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();

        write_pdf_source(&input, &dest).unwrap();

        // 表示順・スライド数は変わらず、音声だけがなくなる
        let map = extract_from_file(&dest).unwrap();
        assert_eq!(map.slides.len(), 4);
        assert!(map.slides.iter().all(|s| !s.has_audio));
        assert_eq!(map.slides[0].advance_sec, Some(5.0));
        // 動画の図形は残るため、動画の警告は元のファイルと同じく出る
        assert_eq!(
            map.warnings,
            vec!["スライド4：動画ナレーションは対象外のため無音として扱います"]
        );
    }
}

#[test]
fn other_entries_are_copied_without_change() {
    let dir = TempDir::new().unwrap();
    let input = lecture(PackageKind::Pptx).write_to(dir.path(), "lecture");
    let dest = dir.path().join("copy.zip");
    write_pdf_source(&input, &dest).unwrap();

    let mut before = open(&input);
    let mut after = open(&dest);
    assert_eq!(names_in_order(&before), names_in_order(&after));

    let rewritten = ["ppt/slides/slide1.xml", "ppt/slides/slide3.xml"];
    for index in 0..before.len() {
        let name = before.name_for_index(index).unwrap().to_owned();
        if rewritten.contains(&name.as_str()) {
            continue;
        }
        // 音声のないスライド（slide2・slide4）・rels・メディアは圧縮済みのバイト列のまま
        assert_eq!(
            raw_bytes(&mut before, index),
            raw_bytes(&mut after, index),
            "{name}"
        );
        let (b, a) = (
            before.by_index(index).unwrap().crc32(),
            after.by_index(index).unwrap().crc32(),
        );
        assert_eq!(b, a, "{name}");
    }
}

#[test]
fn input_file_is_not_modified() {
    let dir = TempDir::new().unwrap();
    let input = lecture(PackageKind::Ppsx).write_to(dir.path(), "lecture");
    let original = fs::read(&input).unwrap();
    let modified = fs::metadata(&input).unwrap().modified().unwrap();

    write_pdf_source(&input, &dir.path().join("copy.zip")).unwrap();

    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::metadata(&input).unwrap().modified().unwrap(), modified);
}

#[test]
fn package_without_audio_is_copied_as_is() {
    let dir = TempDir::new().unwrap();
    let input = PptxBuilder::new()
        .slide(1, SlideBuilder::new().text(2))
        .slide(2, SlideBuilder::new().text(3))
        .write_to(dir.path(), "lecture");
    let dest = dir.path().join("copy.zip");

    assert_eq!(write_pdf_source(&input, &dest).unwrap(), 0);

    let mut before = open(&input);
    let mut after = open(&dest);
    assert_eq!(names_in_order(&before), names_in_order(&after));
    for index in 0..before.len() {
        assert_eq!(raw_bytes(&mut before, index), raw_bytes(&mut after, index));
    }
}

/// 任意のエントリを持つzipを書く（`PptxBuilder` で作れない入力用）
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    fs::write(path, zip.finish().unwrap().into_inner()).unwrap();
}

const AUDIO_SLIDE: &str = r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:pic><p:nvPicPr><p:cNvPr id="4" name="a"/><p:cNvPicPr/><p:nvPr><a:audioFile r:link="rId2"/></p:nvPr></p:nvPicPr></p:pic></p:spTree></p:cSld></p:sld>"#;

#[test]
fn slide_with_bom_is_rewritten_without_bom() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("lecture.pptx");
    let with_bom = format!("\u{feff}{AUDIO_SLIDE}");
    write_zip(&input, &[("ppt/slides/slide1.xml", with_bom.as_bytes())]);
    let dest = dir.path().join("copy.zip");

    assert_eq!(write_pdf_source(&input, &dest).unwrap(), 1);
    let xml = read_entry(&mut open(&dest), "ppt/slides/slide1.xml");
    assert!(!xml.starts_with('\u{feff}'));
    assert!(!xml.contains("audioFile"));
}

#[test]
fn only_slide_parts_are_rewritten() {
    // スライド以外（レイアウト・ノート・スライドの rels 等）に同じ形のXMLがあっても書き換えない
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("lecture.pptx");
    let others = [
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/notesSlides/notesSlide1.xml",
        "ppt/slides/_rels/slide1.xml.rels",
        "ppt/slides/sub/slide1.xml",
    ];
    let mut entries: Vec<(&str, &[u8])> = others
        .iter()
        .map(|name| (*name, AUDIO_SLIDE.as_bytes()))
        .collect();
    entries.push(("ppt/slides/slide1.xml", AUDIO_SLIDE.as_bytes()));
    write_zip(&input, &entries);
    let dest = dir.path().join("copy.zip");

    assert_eq!(write_pdf_source(&input, &dest).unwrap(), 1);
    let mut out = open(&dest);
    for name in others {
        assert_eq!(read_entry(&mut out, name), AUDIO_SLIDE, "{name}");
    }
}

#[test]
fn malformed_slide_is_error_with_part_name() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("lecture.pptx");
    write_zip(
        &input,
        &[(
            "ppt/slides/slide2.xml",
            b"<p:sld><p:cSld></p:sld>".as_slice(),
        )],
    );
    let err = write_pdf_source(&input, &dir.path().join("copy.zip"))
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("スライドのXMLを解析できません"), "{err}");
    assert!(err.contains("ppt/slides/slide2.xml"), "{err}");
    assert!(err.contains("lecture.pptx"), "{err}");
}

#[test]
fn non_utf8_slide_is_error() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("lecture.pptx");
    write_zip(
        &input,
        &[("ppt/slides/slide1.xml", b"<p:sld>\xff</p:sld>".as_slice())],
    );
    let err = write_pdf_source(&input, &dir.path().join("copy.zip"))
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("ppt/slides/slide1.xml をUTF-8として読み込めません"),
        "{err}"
    );
}

#[test]
fn non_zip_input_is_error() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("lecture.pptx");
    fs::write(&input, b"not a zip").unwrap();
    let err = write_pdf_source(&input, &dir.path().join("copy.zip"))
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("pptx/ppsxとして読み込めません"), "{err}");
}

#[test]
fn missing_input_is_error() {
    let dir = TempDir::new().unwrap();
    let err = write_pdf_source(
        &dir.path().join("missing.pptx"),
        &dir.path().join("copy.zip"),
    )
    .unwrap_err()
    .to_string();
    assert!(err.starts_with("ファイルを開けません"), "{err}");
}
