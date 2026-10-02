// ドロップ後の受付判定・上書き確認・状態遷移。DropZone から呼ばれる唯一の入口
import { get, type Writable } from "svelte/store";
import { requestConfirm, type ConfirmRequest } from "./confirmDialog";
import { PipelineOrchestrator } from "./orchestrator";
import { isSupportedInput, resolveOutputPaths } from "./outputPaths";
import { pipelineState } from "./pipelineStore";
import { settingsStatus } from "./settings";
import { AudioConcatStep } from "./steps/audioConcatStep";
import { ExtractPptxStep } from "./steps/extractPptxStep";
import { SlidePdfStep } from "./steps/slidePdfStep";
import { TimestampJsonStep } from "./steps/timestampJsonStep";
import type { PipelineState } from "./steps/types";
import { commands, type AppSettings, type Commands, type SettingsStatus } from "./tauriCommands";

export interface PipelineControllerDeps {
  store: Writable<PipelineState>;
  getSettings: () => SettingsStatus | null;
  commands: Pick<Commands, "checkOutputsExist">;
  confirm: (req: ConfirmRequest) => Promise<boolean>; // 既定は confirmDialog.requestConfirm
  createOrchestrator: (settings: AppSettings) => PipelineOrchestrator;
}

export interface PipelineController {
  startConversion(paths: string[]): Promise<void>;
}

/** パス末尾のファイル名（`\` と `/` の両方を区切りとして扱う） */
function fileName(path: string): string {
  return path.slice(Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/")) + 1);
}

export function createPipelineController(deps: PipelineControllerDeps): PipelineController {
  const { store } = deps;
  // 受付判定〜上書き確認の間も view は idle のままなので、二重起動はこのフラグで防ぐ
  let busy = false;

  async function startConversion(paths: string[]): Promise<void> {
    if (busy || get(store).view !== "idle") return;
    busy = true;
    try {
      await run(paths);
    } finally {
      busy = false;
    }
  }

  async function run(paths: string[]): Promise<void> {
    // 1. 設定が検証済みでなければ受け付けない（設定エラーは DropZone が表示する）
    const status = deps.getSettings();
    if (!status?.ok || !status.settings) return;
    const settings = status.settings;

    // 2. 1ファイル・対応拡張子のみ受け付ける
    if (paths.length !== 1) {
      store.set({ view: "idle", notice: "ファイルは1つずつドロップしてください" });
      return;
    }
    const inputPath = paths[0];
    if (!isSupportedInput(inputPath)) {
      store.set({ view: "idle", notice: "pptx または ppsx ファイルをドロップしてください" });
      return;
    }

    // 3. 出力先に同名ファイルがあれば上書きを確認する
    const outputs = resolveOutputPaths(inputPath);
    let existing: string[];
    try {
      existing = await deps.commands.checkOutputsExist([outputs.audio, outputs.pdf, outputs.json]);
    } catch (e) {
      store.set({ view: "idle", notice: `出力先を確認できませんでした（${String(e)}）` });
      return;
    }
    if (existing.length > 0) {
      const ok = await deps.confirm({
        title: "上書きしますか？",
        message: "出力先に同じ名前のファイルがあります。",
        details: existing.map(fileName),
        confirmLabel: "上書きする",
        cancelLabel: "キャンセル",
      });
      if (!ok) {
        store.set({ view: "idle" });
        return;
      }
    }

    // 4. 処理中に遷移し、ステップの開始・完了を running / results に反映する
    store.set({ view: "processing", inputPath, running: [], results: [] });
    const summary = await deps.createOrchestrator(settings).run(inputPath, outputs, {
      onStart: (name) =>
        store.update((s) => (s.view === "processing" ? { ...s, running: [...s.running, name] } : s)),
      onProgress: (r) =>
        store.update((s) =>
          s.view === "processing"
            ? { ...s, running: s.running.filter((n) => n !== r.stepName), results: [...s.results, r] }
            : s,
        ),
    });

    // 5. 3ステップすべて成功なら done、1つでも失敗なら error
    const current = get(store);
    const results = current.view === "processing" ? current.results : [];
    const { audio, pdf, json } = summary.outputs;
    if (summary.failedSteps.length === 0 && audio && pdf && json) {
      store.set({ view: "done", inputPath, results, outputs: { audio, pdf, json } });
    } else {
      store.set({ view: "error", inputPath, results, outputs: summary.outputs, failedSteps: summary.failedSteps });
    }
  }

  return { startConversion };
}

function createDefaultOrchestrator(settings: AppSettings): PipelineOrchestrator {
  return new PipelineOrchestrator({
    extract: new ExtractPptxStep(),
    audio: new AudioConcatStep(settings),
    pdf: new SlidePdfStep(),
    json: new TimestampJsonStep(),
  });
}

export const pipelineController = createPipelineController({
  store: pipelineState,
  getSettings: () => get(settingsStatus),
  commands,
  confirm: requestConfirm,
  createOrchestrator: createDefaultOrchestrator,
});
