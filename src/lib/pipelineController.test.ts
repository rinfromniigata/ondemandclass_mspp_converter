import { get, writable } from "svelte/store";
import { describe, expect, it, vi } from "vitest";
import type { ConfirmRequest } from "./confirmDialog";
import type { PipelineCallbacks, PipelineOrchestrator, RunSummary } from "./orchestrator";
import { createPipelineController } from "./pipelineController";
import type { OutputPaths, PipelineState, StepResult } from "./steps/types";
import type { AppSettings, SettingsStatus } from "./tauriCommands";

const settings: AppSettings = {
  ffmpegPath: "ffmpeg",
  ffprobePath: "ffprobe",
  sofficePath: "soffice",
  silentSlideHandling: "insert_silence",
  silentSlideDefaultSec: 3,
  audioReencodeOnMismatch: true,
};
const okStatus: SettingsStatus = { ok: true, settingsPath: "app.settings.json", settings, errors: [] };

const input = "/x/lec.pptx";
const paths = { audio: "/x/lec_audio.m4a", pdf: "/x/lec_slides.pdf", json: "/x/lec_timestamps.json" };

function ok(stepName: string): StepResult {
  return { stepName, success: true, message: "ok" };
}

type RunImpl = (inputPath: string, outputs: OutputPaths, cb: PipelineCallbacks) => Promise<RunSummary>;

/** すべて成功する run。途中の store を snapshots に記録する */
function successRun(store: ReturnType<typeof writable<PipelineState>>, snapshots: PipelineState[]): RunImpl {
  return async (_inputPath, outputs, cb) => {
    snapshots.push(get(store));
    cb.onStart("pptx解析");
    snapshots.push(get(store));
    cb.onProgress(ok("pptx解析"));
    cb.onStart("音声結合");
    cb.onStart("PDF変換");
    snapshots.push(get(store));
    cb.onProgress(ok("PDF変換"));
    snapshots.push(get(store));
    cb.onProgress(ok("音声結合"));
    cb.onStart("タイムスタンプ書き出し");
    cb.onProgress(ok("タイムスタンプ書き出し"));
    return { outputs: { audio: outputs.audio, pdf: outputs.pdf, json: outputs.json }, failedSteps: [] };
  };
}

function setup(opts: { status?: SettingsStatus | null; existing?: string[]; confirmResult?: boolean; run?: RunImpl } = {}) {
  const store = writable<PipelineState>({ view: "idle" });
  const snapshots: PipelineState[] = [];
  const run = vi.fn<RunImpl>(opts.run ?? successRun(store, snapshots));
  const createOrchestrator = vi.fn((_s: AppSettings) => ({ run }) as unknown as PipelineOrchestrator);
  const checkOutputsExist = vi.fn(async (_p: string[]) => opts.existing ?? []);
  const confirm = vi.fn(async (_r: ConfirmRequest) => opts.confirmResult ?? true);
  const controller = createPipelineController({
    store,
    getSettings: () => (opts.status === undefined ? okStatus : opts.status),
    commands: { checkOutputsExist },
    confirm,
    createOrchestrator,
  });
  return { store, snapshots, run, createOrchestrator, checkOutputsExist, confirm, controller };
}

describe("pipelineController: 受付判定", () => {
  it.each([
    ["未読み込み", null],
    ["検証エラー", { ok: false, settingsPath: "", settings: null, errors: ["ffmpegPath が存在しません"] }],
  ])("設定が%sなら受け付けず、状態も変えない", async (_label, status) => {
    const { controller, store, checkOutputsExist, run } = setup({ status });
    await controller.startConversion([input]);
    expect(get(store)).toEqual({ view: "idle" });
    expect(checkOutputsExist).not.toHaveBeenCalled();
    expect(run).not.toHaveBeenCalled();
  });

  it.each([[[]], [["/x/a.pptx", "/x/b.pptx"]]])("ファイルが1つでなければ notice 付きの idle にする（%j）", async (dropped) => {
    const { controller, store, run } = setup();
    await controller.startConversion(dropped);
    expect(get(store)).toEqual({ view: "idle", notice: "ファイルは1つずつドロップしてください" });
    expect(run).not.toHaveBeenCalled();
  });

  it("拡張子が pptx / ppsx でなければ notice 付きの idle にする", async () => {
    const { controller, store, checkOutputsExist } = setup();
    await controller.startConversion(["/x/lec.ppt"]);
    expect(get(store)).toEqual({ view: "idle", notice: "pptx または ppsx ファイルをドロップしてください" });
    expect(checkOutputsExist).not.toHaveBeenCalled();
  });

  it("idle 以外のときのドロップは無視する", async () => {
    const { controller, store, checkOutputsExist } = setup();
    const processing: PipelineState = { view: "processing", inputPath: "/x/other.pptx", running: [], results: [] };
    store.set(processing);
    await controller.startConversion([input]);
    expect(get(store)).toBe(processing);
    expect(checkOutputsExist).not.toHaveBeenCalled();
  });

  it("上書き確認中の2回目のドロップは無視する", async () => {
    let answer!: (ok: boolean) => void;
    const { controller, confirm, run } = setup({ existing: [paths.audio] });
    confirm.mockImplementationOnce(() => new Promise<boolean>((r) => (answer = r)));

    const first = controller.startConversion([input]);
    await vi.waitFor(() => expect(confirm).toHaveBeenCalled());
    await controller.startConversion([input]);
    answer(true);
    await first;

    expect(confirm).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledTimes(1);
  });
});

describe("pipelineController: 上書き確認", () => {
  it("出力3ファイルの存在を確認し、既存がなければ確認せずに実行する", async () => {
    const { controller, checkOutputsExist, confirm, run } = setup();
    await controller.startConversion([input]);
    expect(checkOutputsExist).toHaveBeenCalledWith([paths.audio, paths.pdf, paths.json]);
    expect(confirm).not.toHaveBeenCalled();
    expect(run).toHaveBeenCalledWith(input, expect.objectContaining(paths), expect.anything());
  });

  it("既存ファイルがあれば既存ファイル名を列挙して確認する", async () => {
    const { controller, confirm } = setup({ existing: [paths.audio, paths.json] });
    await controller.startConversion([input]);
    expect(confirm).toHaveBeenCalledWith({
      title: "上書きしますか？",
      message: "出力先に同じ名前のファイルがあります。",
      details: ["lec_audio.m4a", "lec_timestamps.json"],
      confirmLabel: "上書きする",
      cancelLabel: "キャンセル",
    });
  });

  it("キャンセルしたら何も実行せず idle に戻る", async () => {
    const { controller, store, createOrchestrator, run } = setup({ existing: [paths.pdf], confirmResult: false });
    await controller.startConversion([input]);
    expect(get(store)).toEqual({ view: "idle" });
    expect(createOrchestrator).not.toHaveBeenCalled();
    expect(run).not.toHaveBeenCalled();
  });

  it("承諾したら設定を渡して Orchestrator を作り、実行する", async () => {
    const { controller, createOrchestrator, run } = setup({ existing: [paths.pdf], confirmResult: true });
    await controller.startConversion([input]);
    expect(createOrchestrator).toHaveBeenCalledWith(settings);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("存在確認が失敗したら理由を notice に入れて idle に戻る", async () => {
    const { controller, store, checkOutputsExist, run } = setup();
    checkOutputsExist.mockRejectedValueOnce("アクセスが拒否されました");
    await controller.startConversion([input]);
    expect(get(store)).toEqual({ view: "idle", notice: "出力先を確認できませんでした（アクセスが拒否されました）" });
    expect(run).not.toHaveBeenCalled();
  });
});

describe("pipelineController: 状態遷移", () => {
  it("processing に遷移し、onStart で running に追加、onProgress で running から除いて results に追記する", async () => {
    const { controller, snapshots } = setup();
    await controller.startConversion([input]);

    expect(snapshots[0]).toEqual({ view: "processing", inputPath: input, running: [], results: [] });
    expect(snapshots[1]).toMatchObject({ running: ["pptx解析"], results: [] });
    expect(snapshots[2]).toMatchObject({ running: ["音声結合", "PDF変換"], results: [ok("pptx解析")] });
    expect(snapshots[3]).toMatchObject({ running: ["音声結合"], results: [ok("pptx解析"), ok("PDF変換")] });
  });

  it("3ステップすべて成功なら done に遷移する", async () => {
    const { controller, store } = setup();
    await controller.startConversion([input]);
    expect(get(store)).toEqual({
      view: "done",
      inputPath: input,
      results: [ok("pptx解析"), ok("PDF変換"), ok("音声結合"), ok("タイムスタンプ書き出し")],
      outputs: paths,
    });
  });

  it("1つでも失敗したら、生成できた成果物と失敗ステップを持って error に遷移する", async () => {
    const pdfFailed: StepResult = { stepName: "PDF変換", success: false, message: "sofficeが失敗しました" };
    const { controller, store } = setup({
      run: async (_i, outputs, cb) => {
        cb.onStart("pptx解析");
        cb.onProgress(ok("pptx解析"));
        cb.onStart("PDF変換");
        cb.onProgress(pdfFailed);
        return { outputs: { audio: outputs.audio, json: outputs.json }, failedSteps: ["PDF変換"] };
      },
    });
    await controller.startConversion([input]);
    expect(get(store)).toEqual({
      view: "error",
      inputPath: input,
      results: [ok("pptx解析"), pdfFailed],
      outputs: { audio: paths.audio, json: paths.json },
      failedSteps: ["PDF変換"],
    });
  });

  it("pptx解析で失敗した場合は成果物なしの error に遷移する", async () => {
    const failed: StepResult = { stepName: "pptx解析", success: false, message: "zipを開けません" };
    const { controller, store } = setup({
      run: async (_i, _o, cb) => {
        cb.onStart("pptx解析");
        cb.onProgress(failed);
        return { outputs: {}, failedSteps: ["pptx解析"] };
      },
    });
    await controller.startConversion([input]);
    expect(get(store)).toEqual({ view: "error", inputPath: input, results: [failed], outputs: {}, failedSteps: ["pptx解析"] });
  });

  it("完了後は再び受け付けられる", async () => {
    const { controller, store, run } = setup();
    await controller.startConversion([input]);
    store.set({ view: "idle" });
    await controller.startConversion([input]);
    expect(run).toHaveBeenCalledTimes(2);
  });
});
