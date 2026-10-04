use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::AppError;

/// rels の `Relationship` 要素1件
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    /// `Target` 属性の値そのまま（正規化は `package::resolve_target` で行う）
    pub target: String,
    /// `TargetMode="External"`（パッケージ外へのリンク）なら true
    pub external: bool,
}

/// `*.rels` のXML文字列を、`Id` をキーにした表にする（純粋関数）。
/// `Id` か `Target` がない `Relationship` は無視する
pub fn parse_rels(xml: &str) -> Result<HashMap<String, Relationship>, AppError> {
    let mut reader = Reader::from_str(xml);
    let mut rels = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e))
                if e.name().local_name().into_inner() == "Relationship" =>
            {
                if let Some(rel) = relationship_of(&e)? {
                    rels.insert(rel.id.clone(), rel);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(invalid_rels(e)),
        }
    }
    Ok(rels)
}

fn relationship_of(e: &BytesStart<'_>) -> Result<Option<Relationship>, AppError> {
    let mut id = None;
    let mut rel_type = String::new();
    let mut target = None;
    let mut external = false;
    for attr in e.attributes() {
        let attr = attr.map_err(invalid_rels)?;
        let value = attr
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(invalid_rels)?
            .into_owned();
        match attr.key.local_name().into_inner() {
            "Id" => id = Some(value),
            "Type" => rel_type = value,
            "Target" => target = Some(value),
            "TargetMode" => external = value.eq_ignore_ascii_case("External"),
            _ => {}
        }
    }
    Ok(match (id, target) {
        (Some(id), Some(target)) => Some(Relationship {
            id,
            rel_type,
            target,
            external,
        }),
        _ => None,
    })
}

fn invalid_rels(e: impl std::fmt::Display) -> AppError {
    AppError::Message(format!("関係ファイル（.rels）を解析できません（{e}）"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLIDE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId3" Type="http://schemas.microsoft.com/office/2007/relationships/media" Target="../media/media1.m4a"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio" Target="file:///C:/Users/x/a&amp;b.m4a" TargetMode="External"/>
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"></Relationship>
</Relationships>"#;

    #[test]
    fn parses_internal_relationship() {
        let rels = parse_rels(SLIDE_RELS).unwrap();
        assert_eq!(rels.len(), 3);
        assert_eq!(
            rels["rId3"],
            Relationship {
                id: "rId3".into(),
                rel_type: "http://schemas.microsoft.com/office/2007/relationships/media".into(),
                target: "../media/media1.m4a".into(),
                external: false,
            }
        );
        // 開始・終了タグの形でも読める
        assert_eq!(rels["rId1"].target, "../slideLayouts/slideLayout1.xml");
        assert!(!rels["rId1"].external);
    }

    #[test]
    fn detects_external_target_mode() {
        let rels = parse_rels(SLIDE_RELS).unwrap();
        assert!(rels["rId2"].external);
        // 属性値のエスケープを解除する
        assert_eq!(rels["rId2"].target, "file:///C:/Users/x/a&b.m4a");
    }

    #[test]
    fn target_mode_internal_is_not_external() {
        let xml = r#"<Relationships><Relationship Id="rId1" Type="t" Target="../media/media1.m4a" TargetMode="Internal"/></Relationships>"#;
        assert!(!parse_rels(xml).unwrap()["rId1"].external);
    }

    #[test]
    fn ignores_relationship_without_id_or_target() {
        let xml = r#"<Relationships>
  <Relationship Type="t" Target="a.xml"/>
  <Relationship Id="rId2" Type="t"/>
  <Relationship Id="rId3" Type="t" Target="b.xml"/>
</Relationships>"#;
        let rels = parse_rels(xml).unwrap();
        assert_eq!(rels.keys().collect::<Vec<_>>(), vec!["rId3"]);
    }

    #[test]
    fn prefixed_element_names_are_accepted() {
        let xml = r#"<r:Relationships xmlns:r="http://schemas.openxmlformats.org/package/2006/relationships"><r:Relationship Id="rId1" Type="t" Target="x.xml"/></r:Relationships>"#;
        assert_eq!(parse_rels(xml).unwrap()["rId1"].target, "x.xml");
    }

    #[test]
    fn empty_rels_is_empty_map() {
        let xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;
        assert!(parse_rels(xml).unwrap().is_empty());
    }

    #[test]
    fn malformed_xml_is_error() {
        let err = parse_rels(r#"<Relationships><Relationship Id="rId1" Target="a"#).unwrap_err();
        assert!(err
            .to_string()
            .starts_with("関係ファイル（.rels）を解析できません"));
    }
}
