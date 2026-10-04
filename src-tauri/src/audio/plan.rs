//! 結合方式（copy / 再エンコード）の事前判定。

use super::probe::AudioInfo;

/// copy 方式で結合できるコーデック（ffprobe の `codec_name`）
const COPY_CODEC: &str = "aac";
/// 再エンコード方式の出力チャンネル数の上限（ステレオ）
const MAX_REENCODE_CHANNELS: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum ConcatMode {
    /// 全音声と同じ形式で無音を生成し、`-c copy` で結合する
    Copy {
        codec: String,
        sample_rate: u32,
        channels: u32,
    },
    /// 各区間を 48kHz・`channels` チャンネルのWAVに正規化してから AAC で結合する
    Reencode { channels: u32 },
}

/// 全音声が AAC で、サンプルレートとチャンネル数が一致すれば `Copy`、それ以外は `Reencode`（純粋関数）。
/// `Reencode` のチャンネル数は最大チャンネル数を 1〜2 に収めた値。
/// `infos` が空の場合（音声区間がない）は `Reencode { channels: 1 }`
pub fn decide_mode(infos: &[AudioInfo]) -> ConcatMode {
    if let Some((first, rest)) = infos.split_first() {
        let copyable = first.codec == COPY_CODEC
            && rest.iter().all(|info| {
                info.codec == first.codec
                    && info.sample_rate == first.sample_rate
                    && info.channels == first.channels
            });
        if copyable {
            return ConcatMode::Copy {
                codec: first.codec.clone(),
                sample_rate: first.sample_rate,
                channels: first.channels,
            };
        }
    }
    let max_channels = infos.iter().map(|info| info.channels).max().unwrap_or(1);
    ConcatMode::Reencode {
        channels: max_channels.clamp(1, MAX_REENCODE_CHANNELS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(codec: &str, sample_rate: u32, channels: u32) -> AudioInfo {
        AudioInfo {
            codec: codec.into(),
            sample_rate,
            channels,
            duration_sec: 1.0,
        }
    }

    #[test]
    fn copy_when_all_aac_with_same_format() {
        let infos = [info("aac", 44100, 2), info("aac", 44100, 2)];
        assert_eq!(
            decide_mode(&infos),
            ConcatMode::Copy {
                codec: "aac".into(),
                sample_rate: 44100,
                channels: 2,
            }
        );
        assert_eq!(
            decide_mode(&[info("aac", 48000, 1)]),
            ConcatMode::Copy {
                codec: "aac".into(),
                sample_rate: 48000,
                channels: 1,
            }
        );
    }

    #[test]
    fn reencode_on_sample_rate_mismatch() {
        let infos = [info("aac", 44100, 1), info("aac", 48000, 1)];
        assert_eq!(decide_mode(&infos), ConcatMode::Reencode { channels: 1 });
    }

    #[test]
    fn reencode_on_channel_mismatch_uses_max_channels() {
        let infos = [info("aac", 48000, 1), info("aac", 48000, 2)];
        assert_eq!(decide_mode(&infos), ConcatMode::Reencode { channels: 2 });
    }

    #[test]
    fn reencode_when_any_codec_is_not_aac() {
        let mixed = [info("aac", 44100, 2), info("mp3", 44100, 2)];
        assert_eq!(decide_mode(&mixed), ConcatMode::Reencode { channels: 2 });
        // 全件が同じ形式でも AAC 以外は copy しない
        let all_mp3 = [info("mp3", 44100, 1), info("mp3", 44100, 1)];
        assert_eq!(decide_mode(&all_mp3), ConcatMode::Reencode { channels: 1 });
    }

    #[test]
    fn reencode_channels_are_capped_at_stereo() {
        let infos = [info("aac", 48000, 6), info("mp3", 48000, 2)];
        assert_eq!(decide_mode(&infos), ConcatMode::Reencode { channels: 2 });
    }

    #[test]
    fn empty_input_falls_back_to_mono_reencode() {
        assert_eq!(decide_mode(&[]), ConcatMode::Reencode { channels: 1 });
    }
}
