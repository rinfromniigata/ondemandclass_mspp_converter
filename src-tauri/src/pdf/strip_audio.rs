//! PDF変換の前に、スライドから音声の図形（PowerPointの再生アイコン）を取り除く。
//!
//! LibreOffice は音声の `p:pic` を、再生アイコンの画像・再生用の注釈・音声データの埋め込みとして
//! PDFに出力する。アイコンが下のテキスト・画像を覆うため、変換前に図形ごと除去する。

use std::ops::Range;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::error::AppError;

/// スライドXMLから音声の図形（`nvPicPr/nvPr` 直下に `audioFile` を持つ `p:pic`）を取り除いた文字列を返す
/// （純粋関数）。該当がなければ `None`。
///
/// - 要素はローカル名で判定し、親要素も確認する（`pptx/slide.rs` と同じ方針）
/// - `mc:AlternateContent` の中の `p:pic` は、分岐ごとにそれぞれ判定して除去する
/// - 動画の図形（`videoFile`）と、`p:timing` 内の `p:spTgt` は残す
pub fn strip_audio_shapes(xml: &str) -> Result<Option<String>, AppError> {
    let ranges = audio_shape_ranges(xml)?;
    if ranges.is_empty() {
        return Ok(None);
    }
    let mut stripped = String::with_capacity(xml.len());
    let mut cursor = 0;
    for range in ranges {
        stripped.push_str(&xml[cursor..range.start]);
        cursor = range.end;
    }
    stripped.push_str(&xml[cursor..]);
    Ok(Some(stripped))
}

/// 音声の `p:pic` の、開始タグの `<` から終了タグの `>` の直後までのバイト範囲（出現順・重なりなし）
pub(crate) fn audio_shape_ranges(xml: &str) -> Result<Vec<Range<usize>>, AppError> {
    let mut reader = Reader::from_str(xml);
    // 開いている要素のローカル名
    let mut stack: Vec<String> = Vec::new();
    // 開いている `pic` ごとに（開始位置, 音声か）
    let mut pics: Vec<(usize, bool)> = Vec::new();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    loop {
        // 直前のイベントの終端 = 次のイベントの開始位置
        let start = to_index(reader.buffer_position());
        match reader.read_event().map_err(invalid_slide)? {
            Event::Start(e) => {
                let name = local_name(&e);
                if name == "pic" {
                    pics.push((start, false));
                }
                mark_if_audio(&name, &stack, &mut pics);
                stack.push(name);
            }
            Event::Empty(e) => {
                mark_if_audio(&local_name(&e), &stack, &mut pics);
            }
            Event::End(_) => {
                if stack.pop().as_deref() == Some("pic") {
                    let (pic_start, is_audio) = pics.pop().expect("pic の開始タグを記録済み");
                    let end = to_index(reader.buffer_position());
                    // 外側の pic に含まれる範囲はあり得ないが、念のため重なりを避ける
                    if is_audio && ranges.last().is_none_or(|last| last.end <= pic_start) {
                        ranges.push(pic_start..end);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(ranges)
}

/// `audioFile` が `pic/nvPicPr/nvPr` の直下にあれば、いちばん内側の `pic` を音声として記録する
fn mark_if_audio(name: &str, stack: &[String], pics: &mut [(usize, bool)]) {
    if name != "audioFile" {
        return;
    }
    let parents: Vec<&str> = stack.iter().rev().take(3).map(String::as_str).collect();
    if parents == ["nvPr", "nvPicPr", "pic"] {
        if let Some(pic) = pics.last_mut() {
            pic.1 = true;
        }
    }
}

fn local_name(e: &BytesStart<'_>) -> String {
    e.name().local_name().into_inner().to_owned()
}

fn to_index(position: u64) -> usize {
    usize::try_from(position).expect("文字列の長さに収まる位置")
}

fn invalid_slide(e: impl std::fmt::Display) -> AppError {
    AppError::Message(format!("スライドのXMLを解析できません（{e}）"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pptx::slide::{parse_slide, MediaKind};

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;

    fn slide(tree: &str, after_tree: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{tree}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>{after_tree}</p:sld>"#
        )
    }

    fn text_shape(id: u32) -> String {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="テキスト {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:p><a:r><a:t>本文 {id}</a:t></a:r></a:p></p:txBody></p:sp>"#
        )
    }

    /// 実ファイル（samples/）と同じ形の音声図形（再生アイコンの画像つき）
    fn audio_shape(id: u32) -> String {
        format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="オーディオ {id}"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:audioFile r:link="rId3"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" r:embed="rId2"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rId7"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="7766304" y="4718304"/><a:ext cx="2057400" cy="2057400"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
        )
    }

    fn video_shape(id: u32) -> String {
        format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="ビデオ {id}"/><p:cNvPicPr/><p:nvPr><a:videoFile r:link="rId4"/></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rId8"/></p:blipFill><p:spPr/></p:pic>"#
        )
    }

    fn image_shape(id: u32) -> String {
        format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="図 {id}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId6"/></p:blipFill><p:spPr/></p:pic>"#
        )
    }

    /// 音声図形を参照する timing（除去後も残す）
    fn timing(spid: u32) -> String {
        format!(
            r#"<p:timing><p:tnLst><p:par><p:cTn id="1" nodeType="tmRoot"><p:childTnLst><p:audio><p:cMediaNode vol="80000"><p:cTn id="2" fill="hold" display="0"/><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl></p:cMediaNode></p:audio></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
        )
    }

    fn strip(xml: &str) -> String {
        strip_audio_shapes(xml)
            .unwrap()
            .expect("音声図形を除去する")
    }

    #[test]
    fn removes_single_audio_shape_like_real_sample() {
        let xml = slide(
            &format!("{}{}{}", text_shape(3), image_shape(4), audio_shape(11)),
            &timing(11),
        );
        let stripped = strip(&xml);
        assert_eq!(
            stripped,
            slide(&format!("{}{}", text_shape(3), image_shape(4)), &timing(11))
        );
        // 除去後もスライドとして読め、音声図形がない
        assert!(parse_slide(&stripped).unwrap().media_shapes.is_empty());
    }

    #[test]
    fn removes_multiple_audio_shapes() {
        let xml = slide(
            &format!(
                "{}{}{}{}",
                audio_shape(4),
                text_shape(3),
                audio_shape(5),
                audio_shape(6)
            ),
            "",
        );
        assert_eq!(strip(&xml), slide(&text_shape(3), ""));
    }

    #[test]
    fn removes_audio_shape_inside_group() {
        let group = |inner: &str| {
            format!(
                r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="20" name="グループ"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{}{inner}</p:grpSp>"#,
                text_shape(21)
            )
        };
        let xml = slide(&group(&audio_shape(22)), "");
        assert_eq!(strip(&xml), slide(&group(""), ""));
    }

    #[test]
    fn slide_without_audio_returns_none() {
        let xml = slide(&format!("{}{}", text_shape(2), image_shape(3)), "");
        assert_eq!(strip_audio_shapes(&xml).unwrap(), None);
    }

    #[test]
    fn video_only_slide_returns_none() {
        let xml = slide(&video_shape(7), "");
        assert_eq!(strip_audio_shapes(&xml).unwrap(), None);
    }

    #[test]
    fn keeps_video_when_mixed_with_audio() {
        let xml = slide(&format!("{}{}", video_shape(7), audio_shape(8)), "");
        let stripped = strip(&xml);
        assert_eq!(stripped, slide(&video_shape(7), ""));
        let parts = parse_slide(&stripped).unwrap();
        assert_eq!(parts.media_shapes.len(), 1);
        assert_eq!(parts.media_shapes[0].kind, MediaKind::Video);
    }

    #[test]
    fn removes_audio_in_every_alternate_branch() {
        let alt = |choice: &str, fallback: &str| {
            format!(
                r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="p14">{choice}</mc:Choice><mc:Fallback>{fallback}</mc:Fallback></mc:AlternateContent>"#
            )
        };
        let xml = slide(
            &alt(
                &format!("{}{}", audio_shape(4), text_shape(5)),
                &audio_shape(4),
            ),
            "",
        );
        assert_eq!(strip(&xml), slide(&alt(&text_shape(5), ""), ""));
    }

    #[test]
    fn audio_file_outside_pic_nv_pr_is_not_removed() {
        // 図形のテキスト内や、nvPr 以外の場所にある同名要素は音声図形として扱わない
        let cases = [
            // p:sp の nvPr 直下（pic ではない）
            r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="a"/><p:cNvSpPr/><p:nvPr><a:audioFile r:link="rId3"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>"#.to_string(),
            // pic 内だが nvPicPr/nvPr の直下ではない
            r#"<p:pic><p:nvPicPr><p:cNvPr id="6" name="b"/><p:cNvPicPr/><p:nvPr><p:extLst><a:audioFile r:link="rId3"/></p:extLst></p:nvPr></p:nvPicPr><p:spPr/></p:pic>"#.to_string(),
            r#"<p:pic><p:nvPicPr><p:cNvPr id="7" name="c"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:audioFile r:link="rId3"/></p:blipFill><p:spPr/></p:pic>"#.to_string(),
        ];
        for shape in cases {
            assert_eq!(
                strip_audio_shapes(&slide(&shape, "")).unwrap(),
                None,
                "{shape}"
            );
        }
    }

    #[test]
    fn audio_file_as_start_tag_is_detected() {
        // <a:audioFile ...></a:audioFile>（空要素でない形）
        let shape = r#"<p:pic><p:nvPicPr><p:cNvPr id="9" name="a"/><p:cNvPicPr/><p:nvPr><a:audioFile r:link="rId3"></a:audioFile></p:nvPr></p:nvPicPr><p:spPr/></p:pic>"#;
        let xml = slide(&format!("{}{shape}", text_shape(2)), "");
        assert_eq!(strip(&xml), slide(&text_shape(2), ""));
    }

    #[test]
    fn other_prefixes_are_accepted() {
        let xml = r#"<x:sld xmlns:x="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:d="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:rel="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><x:cSld><x:spTree><x:pic><x:nvPicPr><x:cNvPr id="4" name="a"/><x:cNvPicPr/><x:nvPr><d:audioFile rel:link="rId2"/></x:nvPr></x:nvPicPr></x:pic></x:spTree></x:cSld></x:sld>"#;
        assert_eq!(
            strip(xml),
            r#"<x:sld xmlns:x="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:d="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:rel="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><x:cSld><x:spTree></x:spTree></x:cSld></x:sld>"#
        );
    }

    #[test]
    fn keeps_surrounding_whitespace_and_non_ascii_text() {
        let xml = slide(
            &format!("\n  {}\n  {}\n", text_shape(3), audio_shape(4)),
            "",
        );
        assert_eq!(
            strip(&xml),
            slide(&format!("\n  {}\n  \n", text_shape(3)), "")
        );
    }

    #[test]
    fn audio_shape_ranges_cover_whole_elements() {
        let xml = slide(&format!("{}{}", audio_shape(4), audio_shape(5)), "");
        let ranges = audio_shape_ranges(&xml).unwrap();
        assert_eq!(ranges.len(), 2);
        assert_eq!(&xml[ranges[0].clone()], audio_shape(4));
        assert_eq!(&xml[ranges[1].clone()], audio_shape(5));
    }

    #[test]
    fn malformed_xml_is_error() {
        for xml in ["<p:sld><p:cSld></p:sld>", "<p:sld><p:pic></p:sld>"] {
            let err = strip_audio_shapes(xml).unwrap_err();
            assert!(
                err.to_string().starts_with("スライドのXMLを解析できません"),
                "{err}"
            );
        }
    }
}
