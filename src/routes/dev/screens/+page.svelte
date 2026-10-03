<!--
  画面の確認用ページ（開発時のみ。/dev/screens）
  Tauri を使わずに、サンプルの状態で ProcessingView・ResultView・ConfirmDialog を表示する
  ?screen=processing-extract|processing|done|partial|failed|dialog、?theme=light|dark で指定する
-->
<script lang="ts">
  import { onMount } from "svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import ProcessingView from "$lib/components/ProcessingView.svelte";
  import ResultView from "$lib/components/ResultView.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { requestConfirm } from "$lib/confirmDialog";
  import type { PipelineState, StepResult } from "$lib/steps/types";

  const SCREENS = ["processing-extract", "processing", "done", "partial", "failed", "dialog"] as const;
  type Screen = (typeof SCREENS)[number];

  const params = new URLSearchParams(location.search);
  const requested = params.get("screen");
  let screen = $state<Screen>(SCREENS.includes(requested as Screen) ? (requested as Screen) : "done");
  const theme = params.get("theme");

  const input = String.raw`C:\Users\example\Documents\講義\第3回 データ構造.pptx`;
  const outputs = {
    audio: String.raw`C:\Users\example\Documents\講義\第3回 データ構造_audio.m4a`,
    pdf: String.raw`C:\Users\example\Documents\講義\第3回 データ構造_slides.pdf`,
    json: String.raw`C:\Users\example\Documents\講義\第3回 データ構造_timestamps.json`,
  };
  const ok = (stepName: string, message: string, warnings?: string[]): StepResult => ({ stepName, success: true, message, warnings });
  const extract = ok("pptx解析", "18枚のスライドを解析しました", ["スライド7：音声ファイルのリンクが切れています"]);

  const samples: Record<Exclude<Screen, "dialog">, PipelineState> = {
    "processing-extract": { view: "processing", inputPath: input, running: ["pptx解析"], results: [], progress: {} },
    processing: {
      view: "processing",
      inputPath: input,
      running: ["音声結合"],
      results: [extract, ok("PDF変換", "PDFに変換しました")],
      progress: {},
    },
    done: {
      view: "done",
      inputPath: input,
      outputs,
      results: [
        extract,
        ok("PDF変換", "PDFに変換しました"),
        ok("音声結合", "音声を結合しました", ["音声の形式がスライド間で異なるため再エンコードで結合しました"]),
        ok("タイムスタンプ書き出し", "タイムスタンプを書き出しました"),
      ],
    },
    partial: {
      view: "error",
      inputPath: input,
      outputs: { pdf: outputs.pdf },
      failedSteps: ["音声結合", "タイムスタンプ書き出し"],
      results: [
        ok("pptx解析", "18枚のスライドを解析しました"),
        ok("PDF変換", "PDFに変換しました"),
        { stepName: "音声結合", success: false, message: "音声を含むスライドがありません" },
        { stepName: "タイムスタンプ書き出し", success: false, message: "音声結合が失敗したため実行しませんでした" },
      ],
    },
    failed: {
      view: "error",
      inputPath: input,
      outputs: {},
      failedSteps: ["pptx解析"],
      results: [{ stepName: "pptx解析", success: false, message: "pptxファイルを開けませんでした（zipとして読み込めません）" }],
    },
  };

  onMount(() => {
    if (theme === "light" || theme === "dark") document.documentElement.setAttribute("data-theme", theme);
    if (screen === "dialog") openDialog();
    return () => document.documentElement.removeAttribute("data-theme");
  });

  function openDialog() {
    void requestConfirm({
      title: "上書きしますか？",
      message: "出力先に同じ名前のファイルがあります。",
      details: ["第3回 データ構造_audio.m4a", "第3回 データ構造_timestamps.json"],
      confirmLabel: "上書きする",
      cancelLabel: "キャンセル",
    });
  }
</script>

<nav class="nav">
  {#each SCREENS as s (s)}
    <Button variant={screen === s ? "filled" : "text"} onclick={() => ((screen = s), s === "dialog" && openDialog())}>{s}</Button>
  {/each}
</nav>

<main class="app">
  {#if screen === "dialog"}
    <ResultView pipeline={samples.done as Extract<PipelineState, { view: "done" }>} />
  {:else}
    {@const state = samples[screen]}
    {#if state.view === "processing"}
      <ProcessingView pipeline={state} />
    {:else if state.view === "done" || state.view === "error"}
      <ResultView pipeline={state} />
    {/if}
  {/if}
</main>
<ConfirmDialog />

<style>
  .nav {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2xs);
    padding: var(--space-xs) var(--space-lg);
    border-bottom: 1px solid var(--color-border);
  }

  /* +page.svelte の .app と同じレイアウト */
  .app {
    display: flex;
    flex-direction: column;
    max-width: 720px;
    margin: 0 auto;
    padding: var(--space-lg);
  }
</style>
