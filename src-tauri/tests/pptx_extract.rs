//! pptx解析の結合テスト（imple 6.1 の主なケース）。
//! `PptxBuilder` で組み立てたファイルを `extract_from_file` / `extract_pptx` に通す

mod common;

use std::path::PathBuf;

use common::fixture::{PackageKind, PptxBuilder, SlideBuilder};
use ondemandclass_mspp_converter_lib::commands::pptx_extract::extract_pptx;
use ondemandclass_mspp_converter_lib::pptx::{extract_from_file, SlideAudioEntry, SlideAudioMap};
use tempfile::TempDir;

fn extract(builder: &PptxBuilder) -> SlideAudioMap {
    let dir = TempDir::new().unwrap();
    let path = builder.write_to(dir.path(), "lecture");
    extract_from_file(&path).unwrap()
}

fn entry(index: u32, file_number: u32, media: &[&str], link_broken: bool) -> SlideAudioEntry {
    SlideAudioEntry {
        slide_index: index,
        slide_xml_path: format!("ppt/slides/slide{file_number}.xml"),
        audio_media_paths: media.iter().map(|m| format!("ppt/media/{m}")).collect(),
        has_audio: !media.is_empty(),
        link_broken,
        advance_sec: None,
    }
}

/// 音声1つのスライド（図形ID 4、rId2 → ../media/{media}）
fn narrated(media: &str) -> SlideBuilder {
    SlideBuilder::new()
        .text(2)
        .audio(4, "rId2")
        .timing_audio(&[4])
        .audio_rel("rId2", &format!("../media/{media}"))
}

/// 受け入れ基準「スライド順序は relsの記載順ではなく presentation.xml の sldIdLst 順」
#[test]
fn display_order_follows_sld_id_list_not_file_numbers_or_rels() {
    // 表示順は slide3 → slide1 → slide2。presentation.xml.rels は 1,2,3 の順に記載される
    let builder = PptxBuilder::new()
        .slide(3, narrated("media3.m4a"))
        .slide(1, narrated("media1.m4a"))
        .slide(2, narrated("media2.m4a"))
        .media("media1.m4a", b"1")
        .media("media2.m4a", b"2")
        .media("media3.m4a", b"3");

    let map = extract(&builder);
    assert_eq!(
        map.slides,
        vec![
            entry(1, 3, &["media3.m4a"], false),
            entry(2, 1, &["media1.m4a"], false),
            entry(3, 2, &["media2.m4a"], false),
        ]
    );
    assert!(map.warnings.is_empty());
}

/// 受け入れ基準「同一スライド内の複数音声は p:timing 順で結合」
#[test]
fn two_audios_follow_timing_order_not_rels_or_tree_order() {
    // rels の記載順・図形ツリー順はどちらも a → b、timing は b → a
    let slide = SlideBuilder::new()
        .audio(4, "rId2")
        .audio(5, "rId3")
        .timing_audio(&[5, 4])
        .audio_rel("rId2", "../media/a.m4a")
        .audio_rel("rId3", "../media/b.m4a");
    let builder = PptxBuilder::new()
        .slide(1, slide)
        .media("a.m4a", b"a")
        .media("b.m4a", b"b");

    let map = extract(&builder);
    assert_eq!(map.slides, vec![entry(1, 1, &["b.m4a", "a.m4a"], false)]);
}

#[test]
fn audio_not_in_timing_is_appended_in_tree_order() {
    // 図形ツリーは 4, 5, 6。timing は 6 のみ
    let slide = SlideBuilder::new()
        .audio(4, "rId2")
        .audio(5, "rId3")
        .audio(6, "rId4")
        .timing_audio(&[6])
        .audio_rel("rId4", "../media/c.m4a")
        .audio_rel("rId3", "../media/b.m4a")
        .audio_rel("rId2", "../media/a.m4a");
    let builder = PptxBuilder::new()
        .slide(1, slide)
        .media("a.m4a", b"a")
        .media("b.m4a", b"b")
        .media("c.m4a", b"c");

    let map = extract(&builder);
    assert_eq!(
        map.slides,
        vec![entry(1, 1, &["c.m4a", "a.m4a", "b.m4a"], false)]
    );
}

#[test]
fn external_and_missing_audio_are_link_broken_with_warning() {
    // スライド1: 外部リンクのみ / スライド2: zip内に実体がない音声と、正常な音声
    let external = SlideBuilder::new()
        .audio(4, "rId2")
        .timing_audio(&[4])
        .external_audio_rel("rId2", "file:///C:/Users/teacher/narration1.m4a");
    let missing = SlideBuilder::new()
        .audio(4, "rId2")
        .audio(5, "rId3")
        .timing_audio(&[4, 5])
        .audio_rel("rId2", "../media/missing.m4a")
        .audio_rel("rId3", "../media/ok.m4a");
    let builder = PptxBuilder::new()
        .slide(1, external)
        .slide(2, missing)
        .media("ok.m4a", b"ok");

    let map = extract(&builder);
    assert_eq!(
        map.slides,
        vec![entry(1, 1, &[], true), entry(2, 2, &["ok.m4a"], true)]
    );
    assert_eq!(
        map.warnings,
        vec![
            "スライド1：音声がリンク切れのため無音として扱います",
            "スライド2：音声がリンク切れのため無音として扱います",
        ]
    );
}

#[test]
fn advance_time_plain_alternate_and_absent() {
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a").advance_ms(32451))
        .slide(2, narrated("a.m4a").advance_ms_alternate(1500, 9000))
        .slide(3, narrated("a.m4a"))
        .media("a.m4a", b"a");

    let advance: Vec<Option<f64>> = extract(&builder)
        .slides
        .iter()
        .map(|s| s.advance_sec)
        .collect();
    assert_eq!(advance, vec![Some(32.451), Some(1.5), None]);
}

#[test]
fn video_shape_is_warned_and_not_treated_as_audio() {
    let slide = SlideBuilder::new()
        .video(4, "rId2")
        .timing_audio(&[4])
        .video_rel("rId2", "../media/movie.mp4");
    let builder = PptxBuilder::new().slide(1, slide).media("movie.mp4", b"v");

    let map = extract(&builder);
    assert_eq!(map.slides, vec![entry(1, 1, &[], false)]);
    assert_eq!(
        map.warnings,
        vec!["スライド1：動画ナレーションは対象外のため無音として扱います"]
    );
}

#[test]
fn slide_without_audio_or_rels() {
    let builder = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .slide(2, SlideBuilder::new().text(2).advance_ms(3000))
        .media("a.m4a", b"a");

    let map = extract(&builder);
    let mut silent = entry(2, 2, &[], false);
    silent.advance_sec = Some(3.0);
    assert_eq!(map.slides, vec![entry(1, 1, &["a.m4a"], false), silent]);
    assert!(map.warnings.is_empty());
}

/// ppsx（`[Content_Types].xml` のメインパートだけが異なる）でも同じ結果になる
#[test]
fn ppsx_gives_same_result_as_pptx() {
    let builder = PptxBuilder::new()
        .slide(2, narrated("b.m4a").advance_ms(5000))
        .slide(1, SlideBuilder::new().text(2))
        .slide(
            3,
            SlideBuilder::new()
                .audio(4, "rId2")
                .external_audio_rel("rId2", "file:///C:/x.m4a"),
        )
        .media("b.m4a", b"b");

    let pptx = extract(&builder);
    let ppsx = extract(&builder.clone().kind(PackageKind::Ppsx));
    assert_eq!(pptx, ppsx);
    assert_eq!(pptx.slides.len(), 3);
}

/// コマンド層：正常時は `SlideAudioMap`、異常時は日本語の文言を返す
#[test]
fn extract_pptx_command_returns_map_or_message() {
    let dir = TempDir::new().unwrap();
    let path = PptxBuilder::new()
        .slide(1, narrated("a.m4a"))
        .media("a.m4a", b"a")
        .kind(PackageKind::Ppsx)
        .write_to(dir.path(), "授業 第1回");

    let map = tauri::async_runtime::block_on(extract_pptx(path.display().to_string())).unwrap();
    assert_eq!(map.slides, vec![entry(1, 1, &["a.m4a"], false)]);

    let not_zip: PathBuf = dir.path().join("broken.pptx");
    std::fs::write(&not_zip, b"not a zip").unwrap();
    let err =
        tauri::async_runtime::block_on(extract_pptx(not_zip.display().to_string())).unwrap_err();
    assert!(err.starts_with("pptx/ppsxとして読み込めません"), "{err}");

    let missing = dir.path().join("none.pptx");
    let err =
        tauri::async_runtime::block_on(extract_pptx(missing.display().to_string())).unwrap_err();
    assert!(err.starts_with("ファイルを開けません"), "{err}");
}
