import { describe, expect, it } from "vitest";
import { isSupportedInput, resolveOutputPaths } from "./outputPaths";

describe("resolveOutputPaths", () => {
  it("Windows形式のパスは入力と同じフォルダに \\ 区切りで出力する", () => {
    expect(resolveOutputPaths(String.raw`C:\lectures\第1回 講義.pptx`)).toEqual({
      dir: String.raw`C:\lectures`,
      basename: "第1回 講義",
      audio: String.raw`C:\lectures\第1回 講義_audio.m4a`,
      pdf: String.raw`C:\lectures\第1回 講義_slides.pdf`,
      json: String.raw`C:\lectures\第1回 講義_timestamps.json`,
    });
  });

  it("POSIX形式のパスは / 区切りで出力する", () => {
    expect(resolveOutputPaths("/Users/a/lec.ppsx")).toEqual({
      dir: "/Users/a",
      basename: "lec",
      audio: "/Users/a/lec_audio.m4a",
      pdf: "/Users/a/lec_slides.pdf",
      json: "/Users/a/lec_timestamps.json",
    });
  });

  it("区切りが混在する場合は最後の区切りで分ける", () => {
    const r = resolveOutputPaths(String.raw`C:\a/b\c.pptx`);
    expect(r.dir).toBe(String.raw`C:\a/b`);
    expect(r.audio).toBe(String.raw`C:\a/b\c_audio.m4a`);
  });

  it("拡張子は大文字小文字を区別せずに除去する", () => {
    expect(resolveOutputPaths("/x/Lec.PPTX").basename).toBe("Lec");
    expect(resolveOutputPaths("/x/Lec.PpSx").basename).toBe("Lec");
  });

  it("ファイル名中の拡張子らしい文字列は末尾以外を残す", () => {
    expect(resolveOutputPaths("/x/a.pptx.backup.pptx").basename).toBe("a.pptx.backup");
  });

  it("区切りのないパスは dir を空にしてファイル名だけで出力する", () => {
    const r = resolveOutputPaths("lec.pptx");
    expect(r.dir).toBe("");
    expect(r.json).toBe("lec_timestamps.json");
  });
});

describe("isSupportedInput", () => {
  it.each(["a.pptx", "a.ppsx", "A.PPTX", String.raw`C:\x\a.Ppsx`])("%s は対応形式", (p) => {
    expect(isSupportedInput(p)).toBe(true);
  });

  it.each(["a.ppt", "a.key", "a.pptx.zip", "a.pdf", "pptx"])("%s は非対応形式", (p) => {
    expect(isSupportedInput(p)).toBe(false);
  });
});
