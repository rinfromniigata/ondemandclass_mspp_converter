//! 結合テスト用の最小構成 pptx/ppsx をメモリ上で組み立てる。
//! 解析の検証用で、PowerPointやLibreOfficeで開ける必要はない

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const REL_AUDIO: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";
const REL_VIDEO: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";

/// スライド1枚分の内容。図形は追加した順に図形ツリーへ並ぶ
#[derive(Clone, Default)]
pub struct SlideBuilder {
    shapes: Vec<String>,
    timing_audio_spids: Vec<u32>,
    transition: Option<String>,
    rels: Vec<String>,
}

impl SlideBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// テキスト図形（音声・動画以外の図形）
    pub fn text(mut self, shape_id: u32) -> Self {
        self.shapes.push(format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="テキスト {shape_id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/></p:sp>"#
        ));
        self
    }

    /// 音声図形（PowerPointの録音と同じく `a:audioFile r:link` と `p14:media r:embed` を持つ）
    pub fn audio(mut self, shape_id: u32, rid: &str) -> Self {
        self.shapes.push(format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{shape_id}" name="オーディオ {shape_id}"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:audioFile r:link="{rid}"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" r:embed="{rid}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill/><p:spPr/></p:pic>"#
        ));
        self
    }

    /// 動画図形
    pub fn video(mut self, shape_id: u32, rid: &str) -> Self {
        self.shapes.push(format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{shape_id}" name="ビデオ {shape_id}"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr/><p:nvPr><a:videoFile r:link="{rid}"/></p:nvPr></p:nvPicPr><p:blipFill/><p:spPr/></p:pic>"#
        ));
        self
    }

    /// `p:timing` に、指定したspidの順で `p:audio` ノードを並べる。
    /// メインシーケンスには実ファイルと同じ playFrom 呼び出しを逆順で入れる（`p:audio` 外なので順序に影響しない）
    pub fn timing_audio(mut self, spids: &[u32]) -> Self {
        self.timing_audio_spids = spids.to_vec();
        self
    }

    /// `p:transition advTm`（ミリ秒）
    pub fn advance_ms(mut self, ms: u32) -> Self {
        self.transition = Some(format!(
            r#"<p:transition spd="med" advTm="{ms}"><p:fade/></p:transition>"#
        ));
        self
    }

    /// 実ファイルと同じく `mc:AlternateContent` で包んだ `p:transition advTm`。
    /// Fallback側には別の値を入れ、Choice側が使われることを確かめられるようにする
    pub fn advance_ms_alternate(mut self, choice_ms: u32, fallback_ms: u32) -> Self {
        self.transition = Some(format!(
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><mc:Choice Requires="p14"><p:transition spd="med" p14:dur="700" advTm="{choice_ms}"><p:fade/></p:transition></mc:Choice><mc:Fallback xmlns=""><p:transition spd="med" advTm="{fallback_ms}"><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"#
        ));
        self
    }

    /// スライドの rels に音声の Relationship を追加する（追加した順に記載される）
    pub fn audio_rel(self, rid: &str, target: &str) -> Self {
        self.rel(rid, REL_AUDIO, target, false)
    }

    pub fn video_rel(self, rid: &str, target: &str) -> Self {
        self.rel(rid, REL_VIDEO, target, false)
    }

    /// `TargetMode="External"` の音声 Relationship
    pub fn external_audio_rel(self, rid: &str, target: &str) -> Self {
        self.rel(rid, REL_AUDIO, target, true)
    }

    fn rel(mut self, rid: &str, rel_type: &str, target: &str, external: bool) -> Self {
        let mode = if external {
            r#" TargetMode="External""#
        } else {
            ""
        };
        self.rels.push(format!(
            r#"<Relationship Id="{rid}" Type="{rel_type}" Target="{target}"{mode}/>"#
        ));
        self
    }

    fn slide_xml(&self) -> String {
        let shapes = self.shapes.concat();
        let transition = self.transition.as_deref().unwrap_or("");
        let timing = if self.timing_audio_spids.is_empty() {
            String::new()
        } else {
            let play_from: String = self
                .timing_audio_spids
                .iter()
                .rev()
                .map(|spid| {
                    format!(
                        r#"<p:par><p:cTn fill="hold"><p:childTnLst><p:cmd type="call" cmd="playFrom(0.0)"><p:cBhvr><p:cTn dur="1" fill="hold"/><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl></p:cBhvr></p:cmd></p:childTnLst></p:cTn></p:par>"#
                    )
                })
                .collect();
            let audio_nodes: String = self
                .timing_audio_spids
                .iter()
                .map(|spid| {
                    format!(
                        r#"<p:audio isNarration="1"><p:cMediaNode vol="80000" showWhenStopped="0"><p:cTn fill="hold" display="0"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl></p:cMediaNode></p:audio>"#
                    )
                })
                .collect();
            format!(
                r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{play_from}</p:childTnLst></p:cTn></p:seq>{audio_nodes}</p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
            )
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>{transition}{timing}</p:sld>"#
        )
    }

    /// Relationship がなければ rels ファイル自体を作らない
    fn rels_xml(&self) -> Option<String> {
        if self.rels.is_empty() {
            return None;
        }
        Some(format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{REL_NS}">{}</Relationships>"#,
            self.rels.concat()
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Pptx,
    Ppsx,
}

impl PackageKind {
    fn extension(self) -> &'static str {
        match self {
            PackageKind::Pptx => "pptx",
            PackageKind::Ppsx => "ppsx",
        }
    }

    fn main_content_type(self) -> &'static str {
        match self {
            PackageKind::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"
            }
            PackageKind::Ppsx => {
                "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml"
            }
        }
    }
}

/// pptx/ppsx 全体。`slide` を呼んだ順が表示順（sldIdLst順）になる。
/// スライドのファイル番号（`slideN.xml` のN）は表示順と独立に指定でき、
/// `presentation.xml.rels` はファイル番号の昇順で記載する
#[derive(Clone)]
pub struct PptxBuilder {
    kind: PackageKind,
    slides: Vec<(u32, SlideBuilder)>,
    media: Vec<(String, Vec<u8>)>,
}

impl Default for PptxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PptxBuilder {
    pub fn new() -> Self {
        Self {
            kind: PackageKind::Pptx,
            slides: Vec::new(),
            media: Vec::new(),
        }
    }

    /// `ppt/slides/slide{file_number}.xml` として、表示順の末尾にスライドを追加する
    pub fn slide(mut self, file_number: u32, slide: SlideBuilder) -> Self {
        self.slides.push((file_number, slide));
        self
    }

    /// `ppt/media/{name}` にメディアの実体を置く
    pub fn media(mut self, name: &str, bytes: &[u8]) -> Self {
        self.media.push((name.to_owned(), bytes.to_vec()));
        self
    }

    /// 種類（`[Content_Types].xml` のメインパートと、`write_to` の拡張子）を切り替える
    pub fn kind(mut self, kind: PackageKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let mut put = |name: &str, data: &[u8]| {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        };

        put("[Content_Types].xml", self.content_types_xml().as_bytes());
        put(
            "_rels/.rels",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{REL_NS}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#
            )
            .as_bytes(),
        );
        put("ppt/presentation.xml", self.presentation_xml().as_bytes());
        put(
            "ppt/_rels/presentation.xml.rels",
            self.presentation_rels_xml().as_bytes(),
        );
        for (number, slide) in &self.slides {
            put(
                &format!("ppt/slides/slide{number}.xml"),
                slide.slide_xml().as_bytes(),
            );
            if let Some(rels) = slide.rels_xml() {
                put(
                    &format!("ppt/slides/_rels/slide{number}.xml.rels"),
                    rels.as_bytes(),
                );
            }
        }
        for (name, bytes) in &self.media {
            put(&format!("ppt/media/{name}"), bytes);
        }
        zip.finish().unwrap().into_inner()
    }

    /// `dir/{stem}.pptx`（ppsxなら `.ppsx`）に書き出してパスを返す
    pub fn write_to(&self, dir: &Path, stem: &str) -> PathBuf {
        let path = dir.join(format!("{stem}.{}", self.kind.extension()));
        std::fs::write(&path, self.to_bytes()).unwrap();
        path
    }

    fn slide_rid(file_number: u32) -> String {
        format!("rId{}", 100 + file_number)
    }

    fn content_types_xml(&self) -> String {
        let overrides: String = self
            .slides
            .iter()
            .map(|(number, _)| {
                format!(
                    r#"<Override PartName="/ppt/slides/slide{number}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
                )
            })
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="m4a" ContentType="audio/mp4"/><Default Extension="mp3" ContentType="audio/mpeg"/><Default Extension="mp4" ContentType="video/mp4"/><Override PartName="/ppt/presentation.xml" ContentType="{}"/>{overrides}</Types>"#,
            self.kind.main_content_type()
        )
    }

    fn presentation_xml(&self) -> String {
        let ids: String = self
            .slides
            .iter()
            .enumerate()
            .map(|(i, (number, _))| {
                format!(
                    r#"<p:sldId id="{}" r:id="{}"/>"#,
                    256 + i,
                    Self::slide_rid(*number)
                )
            })
            .collect();
        // 実ファイルと同じく、セクション情報（r:id なしの p14:sldId）も入れる
        let section_ids: String = (0..self.slides.len())
            .map(|i| format!(r#"<p14:sldId id="{}"/>"#, 256 + i))
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation {NS}><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/><p:extLst><p:ext uri="{{521415D9-36F7-43E2-AB2F-B90AF26B5E84}}"><p14:sectionLst xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><p14:section name="既定のセクション" id="{{00000000-0000-0000-0000-000000000001}}"><p14:sldIdLst>{section_ids}</p14:sldIdLst></p14:section></p14:sectionLst></p:ext></p:extLst></p:presentation>"#
        )
    }

    fn presentation_rels_xml(&self) -> String {
        let mut numbers: Vec<u32> = self.slides.iter().map(|(n, _)| *n).collect();
        numbers.sort_unstable();
        let rels: String = numbers
            .iter()
            .map(|n| {
                format!(
                    r#"<Relationship Id="{}" Type="{REL_SLIDE}" Target="slides/slide{n}.xml"/>"#,
                    Self::slide_rid(*n)
                )
            })
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{REL_NS}">{rels}</Relationships>"#
        )
    }
}
