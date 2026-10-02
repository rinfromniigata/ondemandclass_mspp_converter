import { describe, expect, it, vi } from "vitest";
import type { ConcatResult } from "../tauriCommands";
import { AudioConcatStep, buildSegments, type AudioConcatSettings } from "./audioConcatStep";
import type { SlideAudioEntry } from "./types";

function slide(slideIndex: number, audioMediaPaths: string[], advanceSec: number | null = null, linkBroken = false): SlideAudioEntry {
  return {
    slideIndex,
    slideXmlPath: `ppt/slides/slide${slideIndex}.xml`,
    audioMediaPaths,
    hasAudio: audioMediaPaths.length > 0,
    linkBroken,
    advanceSec,
  };
}

describe("buildSegments", () => {
  const insert = { silentSlideHandling: "insert_silence", silentSlideDefaultSec: 3 } as const;
  const skip = { silentSlideHandling: "skip", silentSlideDefaultSec: 3 } as const;

  it("音声ありスライドは audioMediaPaths の順に media 区間を作る", () => {
    expect(buildSegments([slide(1, ["ppt/media/media2.m4a", "ppt/media/media1.m4a"])], insert)).toEqual([
      { slideIndex: 1, kind: "media", mediaPath: "ppt/media/media2.m4a" },
      { slideIndex: 1, kind: "media", mediaPath: "ppt/media/media1.m4a" },
    ]);
  });

  it("insert_silence では無音スライドに advanceSec 秒の silence 区間を作る", () => {
    expect(buildSegments([slide(1, ["m1"]), slide(2, [], 7.5)], insert)).toEqual([
      { slideIndex: 1, kind: "media", mediaPath: "m1" },
      { slideIndex: 2, kind: "silence", durationSec: 7.5 },
    ]);
  });

  it("advanceSec が null なら silentSlideDefaultSec 秒を使う", () => {
    expect(buildSegments([slide(1, [])], insert)).toEqual([{ slideIndex: 1, kind: "silence", durationSec: 3 }]);
  });

  it("長さが0秒なら区間を作らない", () => {
    expect(buildSegments([slide(1, [], 0)], insert)).toEqual([]);
    expect(buildSegments([slide(1, [])], { ...insert, silentSlideDefaultSec: 0 })).toEqual([]);
  });

  it("リンク切れのスライドは無音スライドとして扱う", () => {
    expect(buildSegments([slide(1, [], null, true)], insert)).toEqual([{ slideIndex: 1, kind: "silence", durationSec: 3 }]);
  });

  it("skip では無音スライドの区間を作らない", () => {
    expect(buildSegments([slide(1, [], 5), slide(2, ["m2"]), slide(3, [])], skip)).toEqual([
      { slideIndex: 2, kind: "media", mediaPath: "m2" },
    ]);
  });

  it("区間はスライドの表示順に並ぶ", () => {
    const segs = buildSegments([slide(1, ["a"]), slide(2, []), slide(3, ["b", "c"])], insert);
    expect(segs.map((s) => s.slideIndex)).toEqual([1, 2, 3, 3]);
  });
});

describe("AudioConcatStep", () => {
  const settings: AudioConcatSettings = {
    silentSlideHandling: "insert_silence",
    silentSlideDefaultSec: 3,
    audioReencodeOnMismatch: true,
  };
  const timestamps = [
    { slide: 1, startSec: 0, endSec: 10 },
    { slide: 2, startSec: 10, endSec: 13 },
  ];

  function step(result: Promise<ConcatResult>) {
    const runFfmpegConcat = vi.fn(() => result);
    return { runFfmpegConcat, step: new AudioConcatStep(settings, { runFfmpegConcat }) };
  }

  it("音声を持つスライドが1枚もなければ ffmpeg を呼ばずに失敗する", async () => {
    const { runFfmpegConcat, step: s } = step(Promise.resolve({ timestamps: [], reencoded: false }));
    const r = await s.execute({ inputPath: "in.pptx", slideAudioMap: { slides: [slide(1, []), slide(2, [], 5)], warnings: [] }, outPath: "o.m4a" });
    expect(r).toMatchObject({ stepName: "音声結合", success: false, message: "音声を含むスライドがありません" });
    expect(runFfmpegConcat).not.toHaveBeenCalled();
  });

  it("全スライドの番号と区間を渡し、timestamps を data として返す", async () => {
    const { runFfmpegConcat, step: s } = step(Promise.resolve({ timestamps, reencoded: false }));
    const r = await s.execute({ inputPath: "in.pptx", slideAudioMap: { slides: [slide(1, ["m1"]), slide(2, [])], warnings: [] }, outPath: "o.m4a" });
    expect(runFfmpegConcat).toHaveBeenCalledWith({
      inputPath: "in.pptx",
      slideIndices: [1, 2],
      segments: [
        { slideIndex: 1, kind: "media", mediaPath: "m1" },
        { slideIndex: 2, kind: "silence", durationSec: 3 },
      ],
      outPath: "o.m4a",
      reencodeOnMismatch: true,
    });
    expect(r).toMatchObject({ success: true, outputPath: "o.m4a", data: { timestamps } });
    expect(r.warnings).toBeUndefined();
  });

  it("再エンコードした場合は警告を載せる", async () => {
    const { step: s } = step(Promise.resolve({ timestamps, reencoded: true }));
    const r = await s.execute({ inputPath: "in.pptx", slideAudioMap: { slides: [slide(1, ["m1"])], warnings: [] }, outPath: "o.m4a" });
    expect(r.success).toBe(true);
    expect(r.warnings).toEqual(["音声の形式がスライド間で異なるため再エンコードで結合しました"]);
  });

  it("invoke の例外は success: false に変換する", async () => {
    const { step: s } = step(Promise.reject("ffmpegの実行に失敗しました"));
    const r = await s.execute({ inputPath: "in.pptx", slideAudioMap: { slides: [slide(1, ["m1"])], warnings: [] }, outPath: "o.m4a" });
    expect(r).toEqual({ stepName: "音声結合", success: false, message: "ffmpegの実行に失敗しました" });
  });
});
