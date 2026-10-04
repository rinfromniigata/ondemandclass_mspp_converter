use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use super::package::{rels_path_of, resolve_target, PptxPackage};
use super::rels::{parse_rels, Relationship};
use crate::error::AppError;

pub const PRESENTATION_PART: &str = "ppt/presentation.xml";

/// `ppt/presentation.xml` の `p:sldIdLst` に従い、表示順のスライドのパート名
/// （例: `ppt/slides/slide3.xml`）を返す。relsの記載順やファイル名の数字は使わない
pub fn slide_order(pkg: &mut PptxPackage) -> Result<Vec<String>, AppError> {
    let presentation_xml = pkg.read_string(PRESENTATION_PART)?;
    let rels_xml = pkg.read_string(&rels_path_of(PRESENTATION_PART))?;
    let rids = parse_slide_rids(&presentation_xml)?;
    let rels = parse_rels(&rels_xml)?;
    resolve_slide_parts(&rids, &rels, |part| pkg.exists(part))
}

/// ルート直下の `p:sldIdLst/p:sldId` の `r:id` を出現順に返す（純粋関数）。
/// `sldIdLst` がない・空の場合と、`r:id` のない `sldId` がある場合はエラー
///
/// セクション情報（`p:extLst` 内の `p14:section/p14:sldIdLst/p14:sldId`）も
/// ローカル名が同じで `r:id` を持たないため、要素の深さでルート直下だけに絞る
pub fn parse_slide_rids(xml: &str) -> Result<Vec<String>, AppError> {
    let mut reader = Reader::from_str(xml);
    // 開いている祖先要素の数（ルート要素の子は深さ1で開始タグを読む）
    let mut depth = 0usize;
    let mut in_list = false;
    let mut rids = Vec::new();
    let push_rid = |e: &BytesStart<'_>, rids: &mut Vec<String>| match relationship_id_of(e)? {
        Some(rid) => {
            rids.push(rid);
            Ok(())
        }
        None => Err(order_error(format!(
            "スライド{}に r:id がありません",
            rids.len() + 1
        ))),
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name().local_name().into_inner();
                if depth == 1 && name == "sldIdLst" {
                    in_list = true;
                } else if in_list && depth == 2 && name == "sldId" {
                    push_rid(&e, &mut rids)?;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if in_list && depth == 2 && e.name().local_name().into_inner() == "sldId" {
                    push_rid(&e, &mut rids)?;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    in_list = false;
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                return Err(order_error(format!(
                    "presentation.xml を解析できません（{e}）"
                )))
            }
        }
    }
    if rids.is_empty() {
        return Err(order_error(
            "presentation.xml のスライド一覧（p:sldIdLst）が空か、ありません".into(),
        ));
    }
    Ok(rids)
}

/// `sldId` の `r:id` を返す。`sldId` にはプレフィックスなしの数値の `id` 属性もあるため、
/// プレフィックス付き（名前空間はrelationships）の `id` だけを対象にする
fn relationship_id_of(e: &BytesStart<'_>) -> Result<Option<String>, AppError> {
    for attr in e.attributes() {
        let attr =
            attr.map_err(|e| order_error(format!("presentation.xml を解析できません（{e}）")))?;
        if attr.key.prefix().is_some() && attr.key.local_name().into_inner() == "id" {
            let value = attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|e| order_error(format!("presentation.xml を解析できません（{e}）")))?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

/// `r:id` の並びを `presentation.xml.rels` でスライドのパート名に変換する（純粋関数）。
/// `exists` でパートがzip内にあるかを確かめる
pub fn resolve_slide_parts(
    rids: &[String],
    rels: &HashMap<String, Relationship>,
    exists: impl Fn(&str) -> bool,
) -> Result<Vec<String>, AppError> {
    rids.iter()
        .enumerate()
        .map(|(i, rid)| {
            let n = i + 1;
            let rel = rels.get(rid).ok_or_else(|| {
                order_error(format!(
                    "スライド{n}の参照 {rid} が presentation.xml.rels にありません"
                ))
            })?;
            if rel.external {
                return Err(order_error(format!(
                    "スライド{n}が外部のファイルを参照しています（{}）",
                    rel.target
                )));
            }
            let part = resolve_target(PRESENTATION_PART, &rel.target);
            if !exists(&part) {
                return Err(order_error(format!(
                    "スライド{n}の {part} がファイル内にありません"
                )));
            }
            Ok(part)
        })
        .collect()
}

fn order_error(detail: String) -> AppError {
    AppError::Message(format!("スライドの順序を解決できません：{detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pptx::package::write_test_zip;

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;

    fn presentation_xml(sld_ids: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation {NS}>
  <p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
  {sld_ids}
  <p:sldSz cx="12192000" cy="6858000"/>
</p:presentation>"#
        )
    }

    /// rId2→slide1、rId3→slide2、rId4→slide3（relsの記載順・ファイル名順は一致）
    const PRESENTATION_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide2.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide3.xml"/>
</Relationships>"#;

    fn rids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_keeps_sld_id_list_order_and_ignores_master_ids() {
        let xml = presentation_xml(
            r#"<p:sldIdLst><p:sldId id="258" r:id="rId4"/><p:sldId id="256" r:id="rId2"/><p:sldId id="257" r:id="rId3"/></p:sldIdLst>"#,
        );
        assert_eq!(
            parse_slide_rids(&xml).unwrap(),
            rids(&["rId4", "rId2", "rId3"])
        );
    }

    /// 実ファイル（samples/）で見つかったケース：セクション情報の `p14:sldIdLst` を読まない
    #[test]
    fn parse_ignores_section_sld_id_lists() {
        let xml = presentation_xml(
            r#"<p:sldIdLst><p:sldId id="256" r:id="rId2"/><p:sldId id="257" r:id="rId3"/></p:sldIdLst>
  <p:extLst>
    <p:ext uri="{521415D9-36F7-43E2-AB2F-B90AF26B5E84}">
      <p14:sectionLst xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main">
        <p14:section name="前半" id="{00000000-0000-0000-0000-000000000001}">
          <p14:sldIdLst><p14:sldId id="256"/></p14:sldIdLst>
        </p14:section>
        <p14:section name="後半" id="{00000000-0000-0000-0000-000000000002}">
          <p14:sldIdLst><p14:sldId id="257"/></p14:sldIdLst>
        </p14:section>
      </p14:sectionLst>
    </p:ext>
  </p:extLst>"#,
        );
        assert_eq!(parse_slide_rids(&xml).unwrap(), rids(&["rId2", "rId3"]));
    }

    #[test]
    fn parse_accepts_other_prefixes_and_attribute_order() {
        let xml = r#"<pr:presentation xmlns:pr="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:rel="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <pr:sldIdLst><pr:sldId rel:id="rId7" id="300"></pr:sldId></pr:sldIdLst>
</pr:presentation>"#;
        assert_eq!(parse_slide_rids(xml).unwrap(), rids(&["rId7"]));
    }

    #[test]
    fn parse_rejects_missing_or_empty_list() {
        for xml in [
            presentation_xml(""),
            presentation_xml("<p:sldIdLst/>"),
            presentation_xml("<p:sldIdLst></p:sldIdLst>"),
        ] {
            let err = parse_slide_rids(&xml).unwrap_err();
            assert_eq!(
                err.to_string(),
                "スライドの順序を解決できません：presentation.xml のスライド一覧（p:sldIdLst）が空か、ありません"
            );
        }
    }

    #[test]
    fn parse_rejects_sld_id_without_relationship_id() {
        let xml = presentation_xml(
            r#"<p:sldIdLst><p:sldId id="256" r:id="rId2"/><p:sldId id="257"/></p:sldIdLst>"#,
        );
        assert_eq!(
            parse_slide_rids(&xml).unwrap_err().to_string(),
            "スライドの順序を解決できません：スライド2に r:id がありません"
        );
    }

    #[test]
    fn parse_rejects_malformed_xml() {
        let err = parse_slide_rids("<p:presentation><p:sldIdLst><p:sldId").unwrap_err();
        assert!(err
            .to_string()
            .starts_with("スライドの順序を解決できません：presentation.xml を解析できません"));
    }

    #[test]
    fn resolve_maps_rids_to_slide_parts() {
        let rels = parse_rels(PRESENTATION_RELS).unwrap();
        let parts = resolve_slide_parts(&rids(&["rId4", "rId2"]), &rels, |_| true).unwrap();
        assert_eq!(
            parts,
            vec!["ppt/slides/slide3.xml", "ppt/slides/slide1.xml"]
        );
    }

    #[test]
    fn resolve_rejects_unknown_rid() {
        let rels = parse_rels(PRESENTATION_RELS).unwrap();
        let err = resolve_slide_parts(&rids(&["rId2", "rId9"]), &rels, |_| true).unwrap_err();
        assert_eq!(
            err.to_string(),
            "スライドの順序を解決できません：スライド2の参照 rId9 が presentation.xml.rels にありません"
        );
    }

    #[test]
    fn resolve_rejects_external_slide() {
        let rels = parse_rels(
            r#"<Relationships><Relationship Id="rId2" Type="t" Target="file:///C:/x.xml" TargetMode="External"/></Relationships>"#,
        )
        .unwrap();
        let err = resolve_slide_parts(&rids(&["rId2"]), &rels, |_| true).unwrap_err();
        assert!(err
            .to_string()
            .contains("スライド1が外部のファイルを参照しています"));
    }

    #[test]
    fn resolve_rejects_missing_part() {
        let rels = parse_rels(PRESENTATION_RELS).unwrap();
        let err = resolve_slide_parts(&rids(&["rId2", "rId3"]), &rels, |part| {
            part != "ppt/slides/slide2.xml"
        })
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "スライドの順序を解決できません：スライド2の ppt/slides/slide2.xml がファイル内にありません"
        );
    }

    #[test]
    fn slide_order_reads_package_in_sld_id_list_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("order.pptx");
        let xml = presentation_xml(
            r#"<p:sldIdLst><p:sldId id="256" r:id="rId3"/><p:sldId id="257" r:id="rId4"/><p:sldId id="258" r:id="rId2"/></p:sldIdLst>"#,
        );
        write_test_zip(
            &path,
            &[
                ("ppt/presentation.xml", xml.as_bytes()),
                (
                    "ppt/_rels/presentation.xml.rels",
                    PRESENTATION_RELS.as_bytes(),
                ),
                ("ppt/slides/slide1.xml", b"<p:sld/>"),
                ("ppt/slides/slide2.xml", b"<p:sld/>"),
                ("ppt/slides/slide3.xml", b"<p:sld/>"),
            ],
        );

        let mut pkg = PptxPackage::open(&path).unwrap();
        assert_eq!(
            slide_order(&mut pkg).unwrap(),
            vec![
                "ppt/slides/slide2.xml",
                "ppt/slides/slide3.xml",
                "ppt/slides/slide1.xml"
            ]
        );
    }

    #[test]
    fn slide_order_fails_without_presentation_xml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.pptx");
        write_test_zip(&path, &[("[Content_Types].xml", b"<Types/>")]);

        let mut pkg = PptxPackage::open(&path).unwrap();
        let err = slide_order(&mut pkg).unwrap_err();
        assert!(err
            .to_string()
            .starts_with("ppt/presentation.xml がファイル内にありません"));
    }
}
