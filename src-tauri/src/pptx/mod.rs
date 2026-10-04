pub mod package;
pub mod presentation;
pub mod rels;
pub mod slide;

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::error::AppError;
use package::{rels_path_of, PptxPackage};
use rels::parse_rels;
use slide::{parse_slide, resolve_audio};

/// 表示順のスライド1枚分の音声情報
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideAudioEntry {
    /// 1始まり、表示順
    pub slide_index: u32,
    /// 例: `ppt/slides/slide3.xml`
    pub slide_xml_path: String,
    /// 再生順のメディアのパート名（リンク切れは含めない）
    pub audio_media_paths: Vec<String>,
    /// `audio_media_paths` が1件以上なら true
    pub has_audio: bool,
    /// リンク切れの音声が1件以上あれば true
    pub link_broken: bool,
    /// `p:transition@advTm` を秒にしたもの。未設定なら `None`（JSONでは null）
    pub advance_sec: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideAudioMap {
    /// 表示順
    pub slides: Vec<SlideAudioEntry>,
    /// リンク切れ・動画ナレーション検出等（スライド番号を含む文言）
    pub warnings: Vec<String>,
}

/// pptx/ppsxを開いて `SlideAudioMap` を作る（zipはディスクに展開しない）
pub fn extract_from_file(path: &Path) -> Result<SlideAudioMap, AppError> {
    let mut pkg = PptxPackage::open(path)?;
    extract_slide_audio_map(&mut pkg)
}

/// スライド順序を解決し、各スライドの音声を再生順に並べて `SlideAudioMap` を組み立てる。
/// リンク切れと動画は警告にとどめ、エラーにしない
pub fn extract_slide_audio_map(pkg: &mut PptxPackage) -> Result<SlideAudioMap, AppError> {
    let slide_parts = presentation::slide_order(pkg)?;
    let mut slides = Vec::with_capacity(slide_parts.len());
    let mut warnings = Vec::new();

    for (i, slide_part) in slide_parts.into_iter().enumerate() {
        let n = i + 1;
        let in_slide = |e: AppError| AppError::Message(format!("スライド{n}（{slide_part}）：{e}"));
        let parts =
            parse_slide(&pkg.read_string(&slide_part).map_err(in_slide)?).map_err(in_slide)?;
        let rels_part = rels_path_of(&slide_part);
        // メディアもレイアウト参照もないスライドには rels がないことがある
        let rels = if pkg.exists(&rels_part) {
            parse_rels(&pkg.read_string(&rels_part).map_err(in_slide)?).map_err(in_slide)?
        } else {
            HashMap::new()
        };
        let audio = resolve_audio(&slide_part, &parts, &rels, |part| pkg.exists(part));

        if audio.link_broken {
            warnings.push(format!(
                "スライド{n}：音声がリンク切れのため無音として扱います"
            ));
        }
        if parts.has_video() {
            warnings.push(format!(
                "スライド{n}：動画ナレーションは対象外のため無音として扱います"
            ));
        }
        slides.push(SlideAudioEntry {
            slide_index: u32::try_from(n).expect("スライド数はu32に収まる"),
            slide_xml_path: slide_part,
            has_audio: !audio.media_paths.is_empty(),
            audio_media_paths: audio.media_paths,
            link_broken: audio.link_broken,
            advance_sec: parts.advance_sec,
        });
    }
    Ok(SlideAudioMap { slides, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pptx::package::write_test_zip;

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;

    const PRESENTATION: &str = r#"<p:presentation xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldIdLst><p:sldId id="256" r:id="rId2"/><p:sldId id="257" r:id="rId3"/><p:sldId id="258" r:id="rId4"/></p:sldIdLst></p:presentation>"#;

    const PRESENTATION_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="slide" Target="slides/slide1.xml"/><Relationship Id="rId3" Type="slide" Target="slides/slide2.xml"/><Relationship Id="rId4" Type="slide" Target="slides/slide3.xml"/></Relationships>"#;

    fn pic(id: u32, element: &str, rid: &str) -> String {
        format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="m{id}"/><p:cNvPicPr/><p:nvPr><a:{element} r:link="{rid}"/></p:nvPr></p:nvPicPr></p:pic>"#
        )
    }

    fn slide(tree: &str, after: &str) -> String {
        format!(r#"<p:sld {NS}><p:cSld><p:spTree>{tree}</p:spTree></p:cSld>{after}</p:sld>"#)
    }

    fn media_rels(entries: &[(&str, &str, bool)]) -> String {
        let items: String = entries
            .iter()
            .map(|(id, target, external)| {
                let mode = if *external {
                    r#" TargetMode="External""#
                } else {
                    ""
                };
                format!(r#"<Relationship Id="{id}" Type="audio" Target="{target}"{mode}/>"#)
            })
            .collect();
        format!(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{items}</Relationships>"#
        )
    }

    #[test]
    fn builds_entries_and_warnings_in_display_order() {
        // スライド1: 音声2つ（timingは 5→4）、advTm 5秒
        let slide1 = slide(
            &format!(
                "{}{}",
                pic(4, "audioFile", "rId2"),
                pic(5, "audioFile", "rId3")
            ),
            r#"<p:transition advTm="5000"/><p:timing><p:tnLst><p:par><p:cTn><p:childTnLst><p:audio><p:cMediaNode><p:cTn/><p:tgtEl><p:spTgt spid="5"/></p:tgtEl></p:cMediaNode></p:audio><p:audio><p:cMediaNode><p:cTn/><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cMediaNode></p:audio></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#,
        );
        let rels1 = media_rels(&[
            ("rId2", "../media/media1.m4a", false),
            ("rId3", "../media/media2.m4a", false),
        ]);
        // スライド2: 音声なし・relsなし
        let slide2 = slide("", "");
        // スライド3: 外部リンクの音声と動画
        let slide3 = slide(
            &format!(
                "{}{}",
                pic(4, "audioFile", "rId2"),
                pic(5, "videoFile", "rId3")
            ),
            "",
        );
        let rels3 = media_rels(&[
            ("rId2", "file:///C:/narration.m4a", true),
            ("rId3", "../media/media3.mp4", false),
        ]);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lecture.ppsx");
        write_test_zip(
            &path,
            &[
                ("ppt/presentation.xml", PRESENTATION.as_bytes()),
                (
                    "ppt/_rels/presentation.xml.rels",
                    PRESENTATION_RELS.as_bytes(),
                ),
                ("ppt/slides/slide1.xml", slide1.as_bytes()),
                ("ppt/slides/_rels/slide1.xml.rels", rels1.as_bytes()),
                ("ppt/slides/slide2.xml", slide2.as_bytes()),
                ("ppt/slides/slide3.xml", slide3.as_bytes()),
                ("ppt/slides/_rels/slide3.xml.rels", rels3.as_bytes()),
                ("ppt/media/media1.m4a", b"a1"),
                ("ppt/media/media2.m4a", b"a2"),
                ("ppt/media/media3.mp4", b"v3"),
            ],
        );

        let map = extract_from_file(&path).unwrap();
        assert_eq!(
            map.slides,
            vec![
                SlideAudioEntry {
                    slide_index: 1,
                    slide_xml_path: "ppt/slides/slide1.xml".into(),
                    audio_media_paths: vec![
                        "ppt/media/media2.m4a".into(),
                        "ppt/media/media1.m4a".into()
                    ],
                    has_audio: true,
                    link_broken: false,
                    advance_sec: Some(5.0),
                },
                SlideAudioEntry {
                    slide_index: 2,
                    slide_xml_path: "ppt/slides/slide2.xml".into(),
                    audio_media_paths: vec![],
                    has_audio: false,
                    link_broken: false,
                    advance_sec: None,
                },
                SlideAudioEntry {
                    slide_index: 3,
                    slide_xml_path: "ppt/slides/slide3.xml".into(),
                    audio_media_paths: vec![],
                    has_audio: false,
                    link_broken: true,
                    advance_sec: None,
                },
            ]
        );
        assert_eq!(
            map.warnings,
            vec![
                "スライド3：音声がリンク切れのため無音として扱います",
                "スライド3：動画ナレーションは対象外のため無音として扱います",
            ]
        );
    }

    #[test]
    fn serializes_with_camel_case_and_null_advance() {
        let map = SlideAudioMap {
            slides: vec![SlideAudioEntry {
                slide_index: 1,
                slide_xml_path: "ppt/slides/slide1.xml".into(),
                audio_media_paths: vec!["ppt/media/media1.m4a".into()],
                has_audio: true,
                link_broken: false,
                advance_sec: None,
            }],
            warnings: vec![],
        };
        assert_eq!(
            serde_json::to_value(&map).unwrap(),
            serde_json::json!({
                "slides": [{
                    "slideIndex": 1,
                    "slideXmlPath": "ppt/slides/slide1.xml",
                    "audioMediaPaths": ["ppt/media/media1.m4a"],
                    "hasAudio": true,
                    "linkBroken": false,
                    "advanceSec": null
                }],
                "warnings": []
            })
        );
    }

    #[test]
    fn broken_slide_xml_names_the_slide() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.pptx");
        write_test_zip(
            &path,
            &[
                ("ppt/presentation.xml", PRESENTATION.as_bytes()),
                (
                    "ppt/_rels/presentation.xml.rels",
                    PRESENTATION_RELS.as_bytes(),
                ),
                ("ppt/slides/slide1.xml", slide("", "").as_bytes()),
                ("ppt/slides/slide2.xml", b"<p:sld><p:cSld></p:sld>"),
                ("ppt/slides/slide3.xml", slide("", "").as_bytes()),
            ],
        );
        let err = extract_from_file(&path).unwrap_err();
        assert!(err
            .to_string()
            .starts_with("スライド2（ppt/slides/slide2.xml）：スライドのXMLを解析できません"));
    }
}
