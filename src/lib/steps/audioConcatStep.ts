// 各スライドの音声と無音区間を1本の m4a に結合する（Rust: run_ffmpeg_concat）
import { commands, type AppSettings, type Commands } from "../tauriCommands";
import type { ActionStep, AudioSegment, SlideAudioEntry, SlideAudioMap, SlideTimestampEntry, StepResult } from "./types";

export type AudioConcatSettings = Pick<
  AppSettings,
  "silentSlideHandling" | "silentSlideDefaultSec" | "audioReencodeOnMismatch"
>;

export interface AudioConcatInput {
  inputPath: string;
  slideAudioMap: SlideAudioMap;
  outPath: string;
}

export interface AudioConcatOutput {
  timestamps: SlideTimestampEntry[];
}

/**
 * スライドの表示順に結合区間を組み立てる（純粋関数）。
 * - 音声ありスライド: audioMediaPaths の順に media 区間
 * - 無音スライド（リンク切れ含む）: insert_silence なら advanceSec ?? 既定秒の silence 区間
 *   （0秒以下なら区間を作らない）、skip なら区間を作らない
 */
export function buildSegments(
  slides: SlideAudioEntry[],
  s: Pick<AppSettings, "silentSlideHandling" | "silentSlideDefaultSec">,
): AudioSegment[] {
  const segments: AudioSegment[] = [];
  for (const slide of slides) {
    if (slide.hasAudio) {
      for (const mediaPath of slide.audioMediaPaths) {
        segments.push({ slideIndex: slide.slideIndex, kind: "media", mediaPath });
      }
    } else if (s.silentSlideHandling === "insert_silence") {
      const durationSec = slide.advanceSec ?? s.silentSlideDefaultSec;
      if (durationSec > 0) {
        segments.push({ slideIndex: slide.slideIndex, kind: "silence", durationSec });
      }
    }
  }
  return segments;
}

export class AudioConcatStep implements ActionStep<AudioConcatInput, AudioConcatOutput> {
  readonly name = "音声結合";

  constructor(
    private readonly settings: AudioConcatSettings,
    private readonly cmd: Pick<Commands, "runFfmpegConcat"> = commands,
  ) {}

  async execute({ inputPath, slideAudioMap, outPath }: AudioConcatInput): Promise<StepResult & { data?: AudioConcatOutput }> {
    try {
      const slides = slideAudioMap.slides;
      if (!slides.some((s) => s.hasAudio)) {
        return { stepName: this.name, success: false, message: "音声を含むスライドがありません" };
      }

      const result = await this.cmd.runFfmpegConcat({
        inputPath,
        slideIndices: slides.map((s) => s.slideIndex),
        segments: buildSegments(slides, this.settings),
        outPath,
        reencodeOnMismatch: this.settings.audioReencodeOnMismatch,
      });
      return {
        stepName: this.name,
        success: true,
        message: "音声を結合しました",
        warnings: result.reencoded ? ["音声の形式がスライド間で異なるため再エンコードで結合しました"] : undefined,
        outputPath: outPath,
        data: { timestamps: result.timestamps },
      };
    } catch (e) {
      return { stepName: this.name, success: false, message: String(e) };
    }
  }
}
