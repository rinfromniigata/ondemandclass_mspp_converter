use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use zip::result::ZipError;
use zip::ZipArchive;

use crate::error::AppError;

/// pptx/ppsx（OPCパッケージ）のzipエントリを読む。
/// パート名はzip内のエントリ名（先頭の `/` なし、`/` 区切り。例: `ppt/slides/slide1.xml`）で指定する
pub struct PptxPackage {
    archive: ZipArchive<File>,
    path: PathBuf,
}

impl PptxPackage {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let file = File::open(path).map_err(|source| AppError::Io {
            context: format!("ファイルを開けません（{}）", path.display()),
            source,
        })?;
        let archive = ZipArchive::new(file).map_err(|e| {
            AppError::Message(format!(
                "pptx/ppsxとして読み込めません（{}：{e}）",
                path.display()
            ))
        })?;
        Ok(Self {
            archive,
            path: path.to_path_buf(),
        })
    }

    pub fn exists(&self, part: &str) -> bool {
        self.archive.index_for_name(part).is_some()
    }

    pub fn read_bytes(&mut self, part: &str) -> Result<Vec<u8>, AppError> {
        let mut entry = match self.archive.by_name(part) {
            Ok(entry) => entry,
            Err(ZipError::FileNotFound) => {
                return Err(AppError::Message(format!(
                    "{part} がファイル内にありません（{}）",
                    self.path.display()
                )))
            }
            Err(e) => {
                return Err(AppError::Message(format!(
                    "{part} を読み込めません（{}：{e}）",
                    self.path.display()
                )))
            }
        };
        let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
        entry
            .read_to_end(&mut bytes)
            .map_err(|source| AppError::Io {
                context: format!("{part} を読み込めません（{}）", self.path.display()),
                source,
            })?;
        Ok(bytes)
    }

    /// XMLパートを文字列で読む。先頭のBOMは取り除く
    pub fn read_string(&mut self, part: &str) -> Result<String, AppError> {
        let bytes = self.read_bytes(part)?;
        let text = String::from_utf8(bytes).map_err(|_| {
            AppError::Message(format!(
                "{part} をUTF-8として読み込めません（{}）",
                self.path.display()
            ))
        })?;
        Ok(match text.strip_prefix('\u{feff}') {
            Some(stripped) => stripped.to_owned(),
            None => text,
        })
    }
}

/// rels の `Target` を、`base_part` を基準にしたパート名へ正規化する（純粋関数）。
/// 例: `ppt/slides/slide3.xml` 基準の `../media/media1.m4a` → `ppt/media/media1.m4a`
///
/// `/` で始まる `Target` はパッケージのルートからの絶対パスとして扱う。
/// ルートより上へ出る `..` は `..` のまま残し、zip内に存在しないパート名にする
pub fn resolve_target(base_part: &str, target: &str) -> String {
    let target = target.replace('\\', "/");
    let mut segments: Vec<&str> = Vec::new();
    if !target.starts_with('/') {
        if let Some((dir, _)) = base_part.rsplit_once('/') {
            segments.extend(dir.split('/').filter(|s| !s.is_empty()));
        }
    }
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => match segments.last() {
                Some(&last) if last != ".." => {
                    segments.pop();
                }
                _ => segments.push(".."),
            },
            _ => segments.push(segment),
        }
    }
    segments.join("/")
}

/// パートに対応する rels のパート名を返す（純粋関数）。
/// 例: `ppt/slides/slide3.xml` → `ppt/slides/_rels/slide3.xml.rels`
pub fn rels_path_of(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, name)) => format!("{dir}/_rels/{name}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// テスト用：`entries`（パート名と中身）を持つzipを `path` に作る
#[cfg(test)]
pub(crate) fn write_test_zip(path: &Path, entries: &[(&str, &[u8])]) {
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, data) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_target_parent_dir() {
        assert_eq!(
            resolve_target("ppt/slides/slide3.xml", "../media/media1.m4a"),
            "ppt/media/media1.m4a"
        );
    }

    #[test]
    fn resolve_target_same_dir_and_dot() {
        assert_eq!(
            resolve_target("ppt/presentation.xml", "slides/slide1.xml"),
            "ppt/slides/slide1.xml"
        );
        assert_eq!(
            resolve_target("ppt/presentation.xml", "./slides/./slide1.xml"),
            "ppt/slides/slide1.xml"
        );
    }

    #[test]
    fn resolve_target_absolute() {
        assert_eq!(
            resolve_target("ppt/slides/slide1.xml", "/ppt/media/media2.mp3"),
            "ppt/media/media2.mp3"
        );
    }

    #[test]
    fn resolve_target_backslash_and_double_slash() {
        assert_eq!(
            resolve_target("ppt/slides/slide1.xml", "..\\media//media1.wav"),
            "ppt/media/media1.wav"
        );
    }

    #[test]
    fn resolve_target_above_root_is_kept() {
        assert_eq!(
            resolve_target("ppt/slides/slide1.xml", "../../../media1.m4a"),
            "../media1.m4a"
        );
    }

    #[test]
    fn resolve_target_root_level_base() {
        assert_eq!(
            resolve_target("[Content_Types].xml", "ppt/presentation.xml"),
            "ppt/presentation.xml"
        );
    }

    #[test]
    fn rels_path_of_nested_and_root_parts() {
        assert_eq!(
            rels_path_of("ppt/slides/slide3.xml"),
            "ppt/slides/_rels/slide3.xml.rels"
        );
        assert_eq!(
            rels_path_of("ppt/presentation.xml"),
            "ppt/_rels/presentation.xml.rels"
        );
        assert_eq!(rels_path_of("root.xml"), "_rels/root.xml.rels");
    }

    #[test]
    fn package_reads_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.pptx");
        write_test_zip(
            &path,
            &[
                (
                    "ppt/presentation.xml",
                    "\u{feff}<p:presentation/>".as_bytes(),
                ),
                ("ppt/media/media1.m4a", &[0, 1, 2, 3]),
            ],
        );

        let mut pkg = PptxPackage::open(&path).unwrap();
        assert!(pkg.exists("ppt/presentation.xml"));
        assert!(pkg.exists("ppt/media/media1.m4a"));
        assert!(!pkg.exists("ppt/media/media2.m4a"));
        assert_eq!(
            pkg.read_string("ppt/presentation.xml").unwrap(),
            "<p:presentation/>"
        );
        assert_eq!(
            pkg.read_bytes("ppt/media/media1.m4a").unwrap(),
            vec![0, 1, 2, 3]
        );

        let err = pkg.read_bytes("ppt/media/media2.m4a").unwrap_err();
        assert!(err
            .to_string()
            .starts_with("ppt/media/media2.m4a がファイル内にありません"));
    }

    #[test]
    fn package_open_rejects_non_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.pptx");
        std::fs::write(&path, b"not a zip").unwrap();

        let err = PptxPackage::open(&path).err().unwrap();
        assert!(err.to_string().starts_with("pptx/ppsxとして読み込めません"));
    }

    #[test]
    fn package_open_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = PptxPackage::open(&dir.path().join("none.pptx"))
            .err()
            .unwrap();
        assert!(matches!(err, AppError::Io { .. }));
    }
}
