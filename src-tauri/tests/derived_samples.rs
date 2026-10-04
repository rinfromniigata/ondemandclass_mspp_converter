//! `scripts/make_samples.ts` が作った派生サンプル（imple 6.4）を `extract_from_file` に通し、
//! 意図した警告・音声なしになっていることを確かめる（T6-2）。
//! 実ファイルは Git で除外しているため `#[ignore]`。`bun scripts/make_samples.ts` の後に、
//! 環境変数 `FFPROBE_PATH` を設定して `cargo test --test derived_samples -- --ignored` で実行する

use std::fs;
use std::path::{Path, PathBuf};

use ondemandclass_mspp_converter_lib::audio::probe::{probe, AudioInfo};
use ondemandclass_mspp_converter_lib::pptx::package::PptxPackage;
use ondemandclass_mspp_converter_lib::pptx::{extract_from_file, SlideAudioMap};
use tempfile::TempDir;

const FORMATS: [&str; 2] = ["pptx", "ppsx"];

// scripts/make_samples.ts の定数と合わせる
const BROKEN_LINK_SLIDE: usize = 3;
const FORMAT_MISMATCH_SLIDE: usize = 2;
const PARTIAL_SILENCE_SLIDES: [usize; 2] = [3, 5];

fn sample(kind: &str, ext: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples/derived")
        .join(ext)
        .join(format!("{kind}.{ext}"))
}

fn extract(kind: &str, ext: &str) -> SlideAudioMap {
    let path = sample(kind, ext);
    extract_from_file(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// 音声のあるスライド番号（1始まり）
fn narrated(map: &SlideAudioMap) -> Vec<usize> {
    map.slides
        .iter()
        .filter(|s| s.has_audio)
        .map(|s| s.slide_index as usize)
        .collect()
}

fn all_slides(map: &SlideAudioMap) -> Vec<usize> {
    (1..=map.slides.len()).collect()
}

/// スライド `n` の最初の音声を一時ファイルに書き出して ffprobe にかける
fn probe_slide(path: &Path, map: &SlideAudioMap, n: usize, dir: &Path) -> AudioInfo {
    let ffprobe = PathBuf::from(
        std::env::var("FFPROBE_PATH")
            .expect("環境変数 FFPROBE_PATH に実行ファイルのパスを設定してください"),
    );
    let part = &map.slides[n - 1].audio_media_paths[0];
    let file = dir.join(format!("slide{n}.m4a"));
    fs::write(
        &file,
        PptxPackage::open(path).unwrap().read_bytes(part).unwrap(),
    )
    .unwrap();
    probe(&ffprobe, &file).unwrap()
}

#[test]
#[ignore]
fn original_has_audio_on_every_slide() {
    for ext in FORMATS {
        let map = extract("original", ext);
        assert_eq!(narrated(&map), all_slides(&map), "{ext}");
        assert!(map.warnings.is_empty(), "{ext}: {:?}", map.warnings);
    }
}

#[test]
#[ignore]
fn broken_link_warns_with_slide_number() {
    for ext in FORMATS {
        let map = extract("broken_link", ext);
        let broken: Vec<usize> = map
            .slides
            .iter()
            .filter(|s| s.link_broken)
            .map(|s| s.slide_index as usize)
            .collect();
        assert_eq!(broken, vec![BROKEN_LINK_SLIDE], "{ext}");
        assert_eq!(
            map.warnings,
            vec![format!(
                "スライド{BROKEN_LINK_SLIDE}：音声がリンク切れのため無音として扱います"
            )],
            "{ext}"
        );
        let expected: Vec<usize> = all_slides(&map)
            .into_iter()
            .filter(|n| *n != BROKEN_LINK_SLIDE)
            .collect();
        assert_eq!(narrated(&map), expected, "{ext}");
    }
}

#[test]
#[ignore]
fn format_mismatch_has_one_part_with_other_sample_rate() {
    let dir = TempDir::new().unwrap();
    for ext in FORMATS {
        let map = extract("format_mismatch", ext);
        assert_eq!(narrated(&map), all_slides(&map), "{ext}");
        assert!(map.warnings.is_empty(), "{ext}: {:?}", map.warnings);

        let path = sample("format_mismatch", ext);
        let reference = probe_slide(&path, &map, 1, dir.path());
        let changed = probe_slide(&path, &map, FORMAT_MISMATCH_SLIDE, dir.path());
        assert_eq!(changed.codec, reference.codec, "{ext}");
        assert_eq!(changed.channels, reference.channels, "{ext}");
        assert_ne!(changed.sample_rate, reference.sample_rate, "{ext}");
    }
}

#[test]
#[ignore]
fn no_audio_has_no_audio_and_no_warning() {
    for ext in FORMATS {
        let map = extract("no_audio", ext);
        assert!(!map.slides.is_empty(), "{ext}");
        assert!(narrated(&map).is_empty(), "{ext}");
        assert!(map.slides.iter().all(|s| !s.link_broken), "{ext}");
        assert!(map.warnings.is_empty(), "{ext}: {:?}", map.warnings);
    }
}

#[test]
#[ignore]
fn partial_silence_drops_audio_of_chosen_slides_only() {
    for ext in FORMATS {
        let map = extract("partial_silence", ext);
        let expected: Vec<usize> = all_slides(&map)
            .into_iter()
            .filter(|n| !PARTIAL_SILENCE_SLIDES.contains(n))
            .collect();
        assert_eq!(narrated(&map), expected, "{ext}");
        assert!(map.slides.iter().all(|s| !s.link_broken), "{ext}");
        assert!(map.warnings.is_empty(), "{ext}: {:?}", map.warnings);
    }
}

#[test]
#[ignore]
fn slide_count_and_advance_times_match_original() {
    for ext in FORMATS {
        let original = extract("original", ext);
        for kind in [
            "broken_link",
            "format_mismatch",
            "no_audio",
            "partial_silence",
        ] {
            let map = extract(kind, ext);
            let advance =
                |m: &SlideAudioMap| m.slides.iter().map(|s| s.advance_sec).collect::<Vec<_>>();
            assert_eq!(advance(&map), advance(&original), "{kind}.{ext}");
        }
    }
}
