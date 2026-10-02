use std::collections::{HashMap, HashSet};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use super::package::resolve_target;
use super::rels::Relationship;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Audio,
    Video,
}

/// 音声・動画を持つ図形1つ
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaShape {
    /// `p:cNvPr@id`（`p:spTgt@spid` と対応する）
    pub shape_id: String,
    pub kind: MediaKind,
    /// `a:audioFile@r:link` / `a:videoFile@r:link`。属性がなければ `None`
    pub link_rid: Option<String>,
}

/// 1枚のスライドXMLから読み取った内容
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlideParts {
    /// 音声・動画の図形（図形ツリーの出現順）
    pub media_shapes: Vec<MediaShape>,
    /// `p:timing` 内の `p:audio` ごとの `p:spTgt@spid`（出現順）
    pub timing_audio_order: Vec<String>,
    /// `p:transition@advTm`（ミリ秒）を秒にしたもの
    pub advance_sec: Option<f64>,
}

impl SlideParts {
    pub fn has_video(&self) -> bool {
        self.media_shapes.iter().any(|s| s.kind == MediaKind::Video)
    }

    /// 音声図形を再生順に並べる。
    /// `p:timing` の spid 順（重複は除く）に並べ、timingに現れない音声図形を図形ツリー順で末尾に追加する
    pub fn ordered_audio_shapes(&self) -> Vec<&MediaShape> {
        let audio: Vec<&MediaShape> = self
            .media_shapes
            .iter()
            .filter(|s| s.kind == MediaKind::Audio)
            .collect();
        let mut used = HashSet::new();
        let mut ordered = Vec::with_capacity(audio.len());
        for spid in &self.timing_audio_order {
            if let Some(shape) = audio.iter().find(|s| &s.shape_id == spid) {
                if used.insert(shape.shape_id.as_str()) {
                    ordered.push(*shape);
                }
            }
        }
        for shape in &audio {
            if used.insert(shape.shape_id.as_str()) {
                ordered.push(*shape);
            }
        }
        ordered
    }
}

/// スライドの音声をrelsで解決した結果
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SlideAudio {
    /// zip内のメディアのパート名（再生順）
    pub media_paths: Vec<String>,
    /// リンク切れの音声が1つ以上あった
    pub link_broken: bool,
}

/// 音声図形の `link_rid` をスライドのrelsで解決する（純粋関数）。
/// relsにない・`TargetMode="External"`・参照先がzip内にない（`exists` が false）・`r:link` がない
/// 場合はリンク切れとし、`media_paths` に含めない
pub fn resolve_audio(
    slide_part: &str,
    parts: &SlideParts,
    rels: &HashMap<String, Relationship>,
    exists: impl Fn(&str) -> bool,
) -> SlideAudio {
    let mut result = SlideAudio::default();
    for shape in parts.ordered_audio_shapes() {
        let resolved = shape
            .link_rid
            .as_ref()
            .and_then(|rid| rels.get(rid))
            .filter(|rel| !rel.external)
            .map(|rel| resolve_target(slide_part, &rel.target))
            .filter(|part| exists(part));
        match resolved {
            Some(part) => result.media_paths.push(part),
            None => result.link_broken = true,
        }
    }
    result
}

/// スライドXMLを解析する（純粋関数）。
///
/// 要素はローカル名で判定するが、同名の要素を取り違えないよう親要素も確認する。
/// - `cNvPr` は `nv*Pr`（`nvPicPr` 等）の直下、`audioFile` / `videoFile` は `nvPr` の直下のみ
/// - `spTgt` は `timing` 内の `audio` の配下のみ（`p:cmd` の playFrom 等は対象外）
/// - `mc:AlternateContent` は最初の分岐（通常は `mc:Choice`）だけを読み、残りは読み飛ばす
pub fn parse_slide(xml: &str) -> Result<SlideParts, AppError> {
    let mut reader = Reader::from_str(xml);
    let mut parser = SlideParser::default();
    loop {
        match reader.read_event().map_err(invalid_slide)? {
            Event::Start(e) => {
                let name = local_name(&e);
                if is_alternate_branch(&name, &parser.stack) {
                    let taken = parser.alt_taken.last_mut().expect("AlternateContent内");
                    if *taken {
                        reader
                            .read_to_end(e.to_end().name())
                            .map_err(invalid_slide)?;
                        continue;
                    }
                    *taken = true;
                }
                parser.on_element(&e, &name)?;
                if name == "AlternateContent" {
                    parser.alt_taken.push(false);
                }
                parser.stack.push(name);
            }
            Event::Empty(e) => {
                let name = local_name(&e);
                if is_alternate_branch(&name, &parser.stack) {
                    let taken = parser.alt_taken.last_mut().expect("AlternateContent内");
                    if *taken {
                        continue;
                    }
                    *taken = true;
                }
                parser.on_element(&e, &name)?;
            }
            Event::End(_) => {
                if parser.stack.pop().as_deref() == Some("AlternateContent") {
                    parser.alt_taken.pop();
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(parser.parts)
}

#[derive(Default)]
struct SlideParser {
    parts: SlideParts,
    /// 開いている要素のローカル名
    stack: Vec<String>,
    /// 開いている `AlternateContent` ごとに、分岐をすでに1つ読んだか
    alt_taken: Vec<bool>,
    /// 直近の `nv*Pr/cNvPr@id`（同じ `nv*Pr` 内の `nvPr` より先に現れる）
    current_shape_id: Option<String>,
    transition_seen: bool,
}

impl SlideParser {
    fn on_element(&mut self, e: &BytesStart<'_>, name: &str) -> Result<(), AppError> {
        let parent = self.stack.last().map(String::as_str);
        match name {
            "cNvPr" if parent.is_some_and(is_non_visual_props) => {
                self.current_shape_id = attr(e, false, "id")?;
            }
            "audioFile" | "videoFile" if parent == Some("nvPr") => {
                self.parts.media_shapes.push(MediaShape {
                    shape_id: self.current_shape_id.clone().unwrap_or_default(),
                    kind: if name == "audioFile" {
                        MediaKind::Audio
                    } else {
                        MediaKind::Video
                    },
                    link_rid: attr(e, true, "link")?,
                });
            }
            "spTgt" if self.has_ancestor("timing") && self.has_ancestor("audio") => {
                if let Some(spid) = attr(e, false, "spid")? {
                    self.parts.timing_audio_order.push(spid);
                }
            }
            "transition" if !self.transition_seen => {
                self.transition_seen = true;
                self.parts.advance_sec = attr(e, false, "advTm")?
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .filter(|ms| ms.is_finite() && *ms >= 0.0)
                    .map(|ms| ms / 1000.0);
            }
            _ => {}
        }
        Ok(())
    }

    fn has_ancestor(&self, name: &str) -> bool {
        self.stack.iter().any(|s| s == name)
    }
}

fn local_name(e: &BytesStart<'_>) -> String {
    e.name().local_name().into_inner().to_owned()
}

/// `nvPicPr` `nvSpPr` `nvGrpSpPr` `nvGraphicFramePr` `nvCxnSpPr` 等（`nvPr` は除く）
fn is_non_visual_props(name: &str) -> bool {
    name != "nvPr" && name.starts_with("nv") && name.ends_with("Pr")
}

fn is_alternate_branch(name: &str, stack: &[String]) -> bool {
    matches!(name, "Choice" | "Fallback")
        && stack.last().map(String::as_str) == Some("AlternateContent")
}

/// 属性値を返す。`prefixed` が true なら `r:link` のようなプレフィックス付き、
/// false ならプレフィックスなしの属性だけを対象にする（プレフィックス名には依存しない）
fn attr(e: &BytesStart<'_>, prefixed: bool, local: &str) -> Result<Option<String>, AppError> {
    for attr in e.attributes() {
        let attr = attr.map_err(invalid_slide)?;
        if attr.key.prefix().is_some() == prefixed && attr.key.local_name().into_inner() == local {
            let value = attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(invalid_slide)?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn invalid_slide(e: impl std::fmt::Display) -> AppError {
    AppError::Message(format!("スライドのXMLを解析できません（{e}）"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pptx::rels::parse_rels;

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;

    fn slide(tree: &str, after_tree: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{tree}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>{after_tree}</p:sld>"#
        )
    }

    fn text_shape(id: u32) -> String {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="テキスト {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/></p:sp>"#
        )
    }

    /// 実ファイル（samples/）と同じ形の音声図形
    fn audio_shape(id: u32, rid: &str) -> String {
        format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="オーディオ {id}"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:audioFile r:link="{rid}"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" r:embed="rIdEmbed{id}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill/><p:spPr/></p:pic>"#
        )
    }

    fn video_shape(id: u32, rid: &str) -> String {
        format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="ビデオ {id}"/><p:cNvPicPr/><p:nvPr><a:videoFile r:link="{rid}"/></p:nvPr></p:nvPicPr><p:blipFill/><p:spPr/></p:pic>"#
        )
    }

    /// `p:audio` ノード（spidごと）と、メインシーケンスの playFrom 呼び出しを含む timing
    fn timing(audio_spids: &[u32], play_from_spids: &[u32]) -> String {
        let calls: String = play_from_spids
            .iter()
            .map(|id| {
                format!(
                    r#"<p:par><p:cTn id="{}"><p:childTnLst><p:cmd type="call" cmd="playFrom(0.0)"><p:cBhvr><p:cTn dur="1"/><p:tgtEl><p:spTgt spid="{id}"/></p:tgtEl></p:cBhvr></p:cmd></p:childTnLst></p:cTn></p:par>"#,
                    100 + id
                )
            })
            .collect();
        let audios: String = audio_spids
            .iter()
            .map(|id| {
                format!(
                    r#"<p:audio isNarration="1"><p:cMediaNode vol="80000" showWhenStopped="0"><p:cTn id="{}" fill="hold" display="0"/><p:tgtEl><p:spTgt spid="{id}"/></p:tgtEl></p:cMediaNode></p:audio>"#,
                    200 + id
                )
            })
            .collect();
        format!(
            r#"<p:timing><p:tnLst><p:par><p:cTn id="1" nodeType="tmRoot"><p:childTnLst><p:seq><p:cTn id="2" nodeType="mainSeq"><p:childTnLst>{calls}</p:childTnLst></p:cTn></p:seq>{audios}</p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
        )
    }

    /// 実ファイルと同じ、AlternateContent で包んだ画面切り替え
    fn alternate_transition(choice_adv: &str, fallback_adv: &str) -> String {
        format!(
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><mc:Choice Requires="p14"><p:transition spd="med" p14:dur="700" {choice_adv}><p:fade/></p:transition></mc:Choice><mc:Fallback xmlns=""><p:transition spd="med" {fallback_adv}><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"#
        )
    }

    fn audio(id: &str, rid: &str) -> MediaShape {
        MediaShape {
            shape_id: id.into(),
            kind: MediaKind::Audio,
            link_rid: Some(rid.into()),
        }
    }

    fn ordered_ids(parts: &SlideParts) -> Vec<&str> {
        parts
            .ordered_audio_shapes()
            .iter()
            .map(|s| s.shape_id.as_str())
            .collect()
    }

    #[test]
    fn parses_slide_like_real_sample() {
        let xml = slide(
            &format!("{}{}", text_shape(3), audio_shape(11, "rId3")),
            &format!(
                "{}{}",
                alternate_transition(r#"advTm="32451""#, r#"advTm="32451""#),
                timing(&[11], &[11])
            ),
        );
        let parts = parse_slide(&xml).unwrap();
        assert_eq!(parts.media_shapes, vec![audio("11", "rId3")]);
        assert_eq!(parts.timing_audio_order, vec!["11"]);
        assert_eq!(parts.advance_sec, Some(32.451));
        assert!(!parts.has_video());
    }

    #[test]
    fn audio_order_follows_timing_not_tree_or_rels() {
        // 図形ツリーは 4→5、timing の p:audio は 5→4
        let xml = slide(
            &format!("{}{}", audio_shape(4, "rId2"), audio_shape(5, "rId3")),
            &timing(&[5, 4], &[]),
        );
        let parts = parse_slide(&xml).unwrap();
        assert_eq!(parts.timing_audio_order, vec!["5", "4"]);
        assert_eq!(ordered_ids(&parts), vec!["5", "4"]);
    }

    #[test]
    fn audio_missing_from_timing_is_appended_in_tree_order() {
        let xml = slide(
            &format!(
                "{}{}{}",
                audio_shape(4, "rId2"),
                audio_shape(5, "rId3"),
                audio_shape(6, "rId4")
            ),
            // spid=6 だけ timing にあり、重複と音声でない図形（3）も含む
            &timing(&[6, 3, 6], &[]),
        );
        let parts = parse_slide(&xml).unwrap();
        assert_eq!(ordered_ids(&parts), vec!["6", "4", "5"]);
    }

    #[test]
    fn play_from_targets_outside_audio_node_are_ignored() {
        let xml = slide(
            &format!("{}{}", audio_shape(4, "rId2"), audio_shape(5, "rId3")),
            &timing(&[], &[5, 4]),
        );
        let parts = parse_slide(&xml).unwrap();
        assert!(parts.timing_audio_order.is_empty());
        assert_eq!(ordered_ids(&parts), vec!["4", "5"]);
    }

    #[test]
    fn detects_video_and_excludes_it_from_audio() {
        let xml = slide(
            &format!("{}{}", video_shape(7, "rId2"), audio_shape(8, "rId3")),
            &timing(&[7, 8], &[]),
        );
        let parts = parse_slide(&xml).unwrap();
        assert!(parts.has_video());
        assert_eq!(parts.media_shapes[0].kind, MediaKind::Video);
        assert_eq!(ordered_ids(&parts), vec!["8"]);
    }

    #[test]
    fn audio_in_group_shape_is_found() {
        let group = format!(
            r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="20" name="グループ"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{}{}</p:grpSp>"#,
            text_shape(21),
            audio_shape(22, "rId5")
        );
        let parts = parse_slide(&slide(&group, "")).unwrap();
        assert_eq!(parts.media_shapes, vec![audio("22", "rId5")]);
    }

    #[test]
    fn audio_file_without_link_has_no_rid() {
        let shape = r#"<p:pic><p:nvPicPr><p:cNvPr id="9" name="a"/><p:cNvPicPr/><p:nvPr><a:audioFile/></p:nvPr></p:nvPicPr></p:pic>"#;
        let parts = parse_slide(&slide(shape, "")).unwrap();
        assert_eq!(parts.media_shapes[0].shape_id, "9");
        assert_eq!(parts.media_shapes[0].link_rid, None);
    }

    #[test]
    fn only_first_alternate_branch_is_read() {
        // Choice と Fallback の両方に同じ音声図形がある場合、1つとして数える
        let alt = format!(
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="p14">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
            audio_shape(4, "rId2"),
            audio_shape(4, "rId2")
        );
        let parts = parse_slide(&slide(&alt, "")).unwrap();
        assert_eq!(parts.media_shapes, vec![audio("4", "rId2")]);
    }

    #[test]
    fn advance_time_variants() {
        let cases = [
            (r#"<p:transition advTm="5000"/>"#.to_string(), Some(5.0)),
            (
                r#"<p:transition spd="slow" advTm="1500"><p:fade/></p:transition>"#.to_string(),
                Some(1.5),
            ),
            (
                alternate_transition(r#"advTm="2000""#, r#"advTm="9000""#),
                Some(2.0),
            ),
            (r#"<p:transition spd="fast"/>"#.to_string(), None),
            (r#"<p:transition advTm="abc"/>"#.to_string(), None),
            (String::new(), None),
        ];
        for (transition, expected) in cases {
            let parts = parse_slide(&slide("", &transition)).unwrap();
            assert_eq!(parts.advance_sec, expected, "{transition}");
        }
    }

    #[test]
    fn other_prefixes_are_accepted() {
        let xml = r#"<x:sld xmlns:x="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:d="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:rel="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><x:cSld><x:spTree><x:pic><x:nvPicPr><x:cNvPr id="4" name="a"/><x:cNvPicPr/><x:nvPr><d:audioFile rel:link="rId2"/></x:nvPr></x:nvPicPr></x:pic></x:spTree></x:cSld><x:transition advTm="3000"/><x:timing><x:tnLst><x:par><x:cTn><x:childTnLst><x:audio><x:cMediaNode><x:cTn/><x:tgtEl><x:spTgt spid="4"/></x:tgtEl></x:cMediaNode></x:audio></x:childTnLst></x:cTn></x:par></x:tnLst></x:timing></x:sld>"#;
        let parts = parse_slide(xml).unwrap();
        assert_eq!(parts.media_shapes, vec![audio("4", "rId2")]);
        assert_eq!(parts.timing_audio_order, vec!["4"]);
        assert_eq!(parts.advance_sec, Some(3.0));
    }

    #[test]
    fn slide_without_media() {
        let parts = parse_slide(&slide(&text_shape(2), "")).unwrap();
        assert_eq!(parts, SlideParts::default());
    }

    #[test]
    fn malformed_xml_is_error() {
        let err = parse_slide("<p:sld><p:cSld></p:sld>").unwrap_err();
        assert!(err.to_string().starts_with("スライドのXMLを解析できません"));
    }

    const SLIDE_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio" Target="../media/media1.m4a"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio" Target="../media/media2.m4a"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio" Target="file:///C:/narration.m4a" TargetMode="External"/>
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio" Target="../media/missing.m4a"/>
</Relationships>"#;

    fn parts_with(shapes: Vec<MediaShape>, timing: &[&str]) -> SlideParts {
        SlideParts {
            media_shapes: shapes,
            timing_audio_order: timing.iter().map(|s| s.to_string()).collect(),
            advance_sec: None,
        }
    }

    fn exists_except_missing(part: &str) -> bool {
        part != "ppt/media/missing.m4a"
    }

    #[test]
    fn resolve_audio_in_timing_order() {
        let rels = parse_rels(SLIDE_RELS).unwrap();
        let parts = parts_with(vec![audio("4", "rId2"), audio("5", "rId3")], &["5", "4"]);
        let result = resolve_audio(
            "ppt/slides/slide1.xml",
            &parts,
            &rels,
            exists_except_missing,
        );
        assert_eq!(
            result,
            SlideAudio {
                media_paths: vec!["ppt/media/media2.m4a".into(), "ppt/media/media1.m4a".into()],
                link_broken: false,
            }
        );
    }

    #[test]
    fn resolve_audio_marks_broken_links() {
        let rels = parse_rels(SLIDE_RELS).unwrap();
        let mut no_link = audio("8", "");
        no_link.link_rid = None;
        let broken_cases = [
            audio("4", "rId9"), // relsにない
            audio("4", "rId4"), // External
            audio("4", "rId5"), // zip内にない
            no_link,            // r:link なし
        ];
        for shape in broken_cases {
            let parts = parts_with(vec![shape.clone(), audio("6", "rId2")], &[]);
            let result = resolve_audio(
                "ppt/slides/slide1.xml",
                &parts,
                &rels,
                exists_except_missing,
            );
            assert_eq!(
                result,
                SlideAudio {
                    media_paths: vec!["ppt/media/media1.m4a".into()],
                    link_broken: true,
                },
                "{shape:?}"
            );
        }
    }

    #[test]
    fn resolve_audio_without_audio_shapes() {
        let rels = parse_rels(SLIDE_RELS).unwrap();
        let parts = parts_with(
            vec![MediaShape {
                shape_id: "7".into(),
                kind: MediaKind::Video,
                link_rid: Some("rId2".into()),
            }],
            &[],
        );
        let result = resolve_audio("ppt/slides/slide1.xml", &parts, &rels, |_| true);
        assert_eq!(result, SlideAudio::default());
    }
}
