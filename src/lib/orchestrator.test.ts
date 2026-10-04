import { describe, expect, it, vi } from "vitest";
import { PipelineOrchestrator, type PipelineCallbacks, type PipelineSteps } from "./orchestrator";
import type { OutputPaths, StepResult } from "./steps/types";

const outputs: OutputPaths = {
  dir: "/x",
  basename: "lec",
  audio: "/x/lec_audio.m4a",
  pdf: "/x/lec_slides.pdf",
  json: "/x/lec_timestamps.json",
};
const slideAudioMap = { slides: [], warnings: [] };
const timestamps = [{ slide: 1, startSec: 0, endSec: 5 }];

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

function result<T>(stepName: string, success: boolean, data?: T): StepResult & { data?: T } {
  return { stepName, success, message: success ? "ok" : "ng", data };
}

/** 各ステップの execute をモック化し、呼び出しとコールバックを1本のログに記録する */
function setup(opts: { extract?: boolean; audio?: boolean; pdf?: boolean; json?: boolean } = {}) {
  const { extract = true, audio = true, pdf = true, json = true } = opts;
  const log: string[] = [];
  const mock = <I, O>(name: string, ok: boolean, data?: O) => ({
    name,
    execute: vi.fn(async (_input: I) => {
      log.push(`exec:${name}`);
      return result<O>(name, ok, ok ? data : undefined);
    }),
  });
  const steps = {
    extract: mock("pptx解析", extract, slideAudioMap),
    audio: mock("音声結合", audio, { timestamps }),
    pdf: mock("PDF変換", pdf, outputs.pdf),
    json: mock("タイムスタンプ書き出し", json, outputs.json),
  };
  const cb: PipelineCallbacks = {
    onStart: (n) => log.push(`start:${n}`),
    onProgress: (r) => log.push(`done:${r.stepName}:${r.success ? "ok" : "ng"}`),
    onStepProgress: (n, ratio) => log.push(`progress:${n}:${ratio}`),
  };
  return { steps, cb, log, orchestrator: new PipelineOrchestrator(steps as unknown as PipelineSteps) };
}

describe("PipelineOrchestrator", () => {
  it("全ステップ成功時は onStart → execute → onProgress の順で呼び、成果物を3つ返す", async () => {
    const { orchestrator, cb, log, steps } = setup();
    const summary = await orchestrator.run("/x/lec.pptx", outputs, cb);

    expect(log).toEqual([
      "start:pptx解析",
      "exec:pptx解析",
      "done:pptx解析:ok",
      "start:音声結合",
      "progress:音声結合:0",
      "start:PDF変換",
      "exec:音声結合",
      "exec:PDF変換",
      "done:音声結合:ok",
      "done:PDF変換:ok",
      "start:タイムスタンプ書き出し",
      "exec:タイムスタンプ書き出し",
      "done:タイムスタンプ書き出し:ok",
    ]);
    expect(summary).toEqual({
      outputs: { audio: outputs.audio, pdf: outputs.pdf, json: outputs.json },
      failedSteps: [],
    });
    expect(steps.audio.execute).toHaveBeenCalledWith({
      inputPath: "/x/lec.pptx",
      slideAudioMap,
      outPath: outputs.audio,
      onProgress: expect.any(Function),
    });
    expect(steps.pdf.execute).toHaveBeenCalledWith({ inputPath: "/x/lec.pptx", outPath: outputs.pdf });
    expect(steps.json.execute).toHaveBeenCalledWith({ timestamps, outPath: outputs.json });
  });

  it("pptx解析が失敗したら後続を実行せず、成果物なしで返す", async () => {
    const { orchestrator, cb, log, steps } = setup({ extract: false });
    const summary = await orchestrator.run("/x/lec.pptx", outputs, cb);

    expect(log).toEqual(["start:pptx解析", "exec:pptx解析", "done:pptx解析:ng"]);
    expect(summary).toEqual({ outputs: {}, failedSteps: ["pptx解析"] });
    expect(steps.audio.execute).not.toHaveBeenCalled();
    expect(steps.pdf.execute).not.toHaveBeenCalled();
    expect(steps.json.execute).not.toHaveBeenCalled();
  });

  it("音声結合が失敗したら JSON は onStart も execute もせず失敗結果だけ通知する。PDF は残る", async () => {
    const { orchestrator, cb, log, steps } = setup({ audio: false });
    const progress: StepResult[] = [];
    const summary = await orchestrator.run("/x/lec.pptx", outputs, { ...cb, onProgress: (r) => (cb.onProgress(r), progress.push(r)) });

    expect(steps.json.execute).not.toHaveBeenCalled();
    expect(log).not.toContain("start:タイムスタンプ書き出し");
    expect(progress.at(-1)).toEqual({
      stepName: "タイムスタンプ書き出し",
      success: false,
      message: "音声結合が失敗したため実行しませんでした",
    });
    expect(summary).toEqual({ outputs: { pdf: outputs.pdf }, failedSteps: ["音声結合", "タイムスタンプ書き出し"] });
  });

  it("PDF変換が失敗しても音声と JSON の成果物は残る", async () => {
    const { orchestrator, cb } = setup({ pdf: false });
    const summary = await orchestrator.run("/x/lec.pptx", outputs, cb);
    expect(summary).toEqual({ outputs: { audio: outputs.audio, json: outputs.json }, failedSteps: ["PDF変換"] });
  });

  it("JSON 書き出しが失敗した場合は failedSteps に加える", async () => {
    const { orchestrator, cb } = setup({ json: false });
    const summary = await orchestrator.run("/x/lec.pptx", outputs, cb);
    expect(summary).toEqual({ outputs: { audio: outputs.audio, pdf: outputs.pdf }, failedSteps: ["タイムスタンプ書き出し"] });
  });

  it("音声結合の進捗だけを、音声結合の名前で onStepProgress へ通知する", async () => {
    const { orchestrator, cb, log, steps } = setup();
    steps.audio.execute.mockImplementationOnce(async (input: unknown) => {
      log.push("exec:音声結合");
      const { onProgress } = input as { onProgress?: (ratio: number) => void };
      onProgress?.(0.4);
      onProgress?.(1);
      return result("音声結合", true, { timestamps });
    });

    await orchestrator.run("/x/lec.pptx", outputs, cb);

    expect(log.filter((l) => l.startsWith("progress:"))).toEqual([
      "progress:音声結合:0",
      "progress:音声結合:0.4",
      "progress:音声結合:1",
    ]);
    // 開始直後の 0 は、音声結合の onStart の直後・execute より前に通知する
    expect(log.indexOf("progress:音声結合:0")).toBe(log.indexOf("start:音声結合") + 1);
    expect(log.indexOf("progress:音声結合:0")).toBeLessThan(log.indexOf("exec:音声結合"));
    // ほかのステップには進捗の通知先を渡さない
    expect(steps.pdf.execute.mock.calls[0][0]).not.toHaveProperty("onProgress");
    expect(steps.json.execute.mock.calls[0][0]).not.toHaveProperty("onProgress");
  });

  it("pptx解析が失敗した場合は進捗を通知しない", async () => {
    const { orchestrator, cb, log } = setup({ extract: false });
    await orchestrator.run("/x/lec.pptx", outputs, cb);
    expect(log.some((l) => l.startsWith("progress:"))).toBe(false);
  });

  it("音声結合と PDF 変換は並列に実行し、完了した順に onProgress へ通知する", async () => {
    const { orchestrator, cb, log, steps } = setup();
    const audioDone = deferred<StepResult & { data?: { timestamps: typeof timestamps } }>();
    const pdfDone = deferred<StepResult & { data?: string }>();
    steps.audio.execute.mockImplementationOnce(() => audioDone.promise);
    steps.pdf.execute.mockImplementationOnce(() => pdfDone.promise);

    const running = orchestrator.run("/x/lec.pptx", outputs, cb);
    await vi.waitFor(() => expect(steps.pdf.execute).toHaveBeenCalled());
    // 音声結合が終わる前に PDF 変換も開始している
    expect(steps.audio.execute).toHaveBeenCalled();

    pdfDone.resolve(result("PDF変換", true, outputs.pdf));
    await vi.waitFor(() => expect(log).toContain("done:PDF変換:ok"));
    expect(log).not.toContain("done:音声結合:ok");

    audioDone.resolve(result("音声結合", true, { timestamps }));
    await running;
    expect(log.indexOf("done:PDF変換:ok")).toBeLessThan(log.indexOf("done:音声結合:ok"));
  });
});
