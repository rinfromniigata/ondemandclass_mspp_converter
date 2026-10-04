//! 一時フォルダで作った成果物を出力先へ移すためのファイル操作。

use std::fs;
use std::io;
use std::path::Path;

/// `from` を `to` へ移す（既存の `to` は置き換える）。ドライブが異なる等で rename できない場合はコピーする。
/// コピーした場合 `from` は残る（呼び出し側の一時フォルダごと削除する前提）
pub fn move_or_copy(from: &Path, to: &Path) -> io::Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    fs::copy(from, to).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn replaces_existing_destination() {
        let dir = TempDir::new().unwrap();
        let from = dir.path().join("from.bin");
        let to = dir.path().join("to.bin");
        fs::write(&from, b"new").unwrap();
        fs::write(&to, b"old").unwrap();
        move_or_copy(&from, &to).unwrap();
        assert_eq!(fs::read(&to).unwrap(), b"new");
        assert!(!from.exists());
    }

    #[test]
    fn missing_source_is_error() {
        let dir = TempDir::new().unwrap();
        assert!(move_or_copy(&dir.path().join("none"), &dir.path().join("to")).is_err());
    }
}
