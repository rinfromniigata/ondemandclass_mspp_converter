<!--
  done / error 画面。状態チップ、成果物一覧、失敗・警告の Banner、操作ボタン、処理ログ（ログ詳細）を表示する
-->
<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { fileNameOf } from "../outputPaths";
  import { pipelineState } from "../pipelineStore";
  import type { PipelineState } from "../steps/types";
  import StepLog from "./StepLog.svelte";
  import Banner from "./ui/Banner.svelte";
  import Button from "./ui/Button.svelte";
  import Card from "./ui/Card.svelte";
  import StatusChip from "./ui/StatusChip.svelte";
  import FileIcon from "./ui/icons/FileIcon.svelte";
  import FolderIcon from "./ui/icons/FolderIcon.svelte";

  let { pipeline }: { pipeline: Extract<PipelineState, { view: "done" | "error" }> } = $props();

  const OUTPUT_LABELS = { audio: "結合音声", pdf: "スライドPDF", json: "タイムスタンプ" } as const;

  const outputs = $derived(
    (["audio", "pdf", "json"] as const).flatMap((kind) => {
      const path = pipeline.outputs[kind];
      return path ? [{ kind, label: OUTPUT_LABELS[kind], path }] : [];
    }),
  );
  const chip = $derived(pipeline.view === "done" ? "done" : outputs.length > 0 ? "partial" : "failed");
  const failures = $derived(pipeline.results.filter((r) => !r.success));
  const warned = $derived(pipeline.results.filter((r) => r.success && r.warnings && r.warnings.length > 0));

  let showLog = $state(false);
  let revealError = $state<string | null>(null);

  async function openFolder() {
    revealError = null;
    try {
      await revealItemInDir(outputs[0].path);
    } catch (e) {
      revealError = String(e);
    }
  }
</script>

<div class="result">
  <header class="header">
    <div class="heading">
      <StatusChip status={chip} />
      <p class="mono muted">{fileNameOf(pipeline.inputPath)}</p>
    </div>
    {#if outputs.length > 0}
      <!-- 仕分けトレイのモチーフ：1つの入力から3つの出力に分かれる -->
      <svg class="tray" width="96" height="48" viewBox="0 0 96 48" aria-hidden="true" focusable="false">
        <rect x="4" y="16" width="20" height="16" rx="3" />
        <path d="M24 24h12M36 24c8 0 8-16 16-16h8M36 24h24M36 24c8 0 8 16 16 16h8" />
        <rect x="60" y="2" width="32" height="12" rx="3" />
        <rect x="60" y="18" width="32" height="12" rx="3" />
        <rect x="60" y="34" width="32" height="12" rx="3" />
      </svg>
    {/if}
  </header>

  {#each failures as r, i (i)}
    <Banner role="alert" title="{r.stepName}が失敗しました">
      <strong class="message">{r.message}</strong>
    </Banner>
  {/each}

  {#each warned as r, i (i)}
    <Banner role="status" title="{r.stepName}の警告" items={r.warnings} />
  {/each}

  {#if outputs.length > 0}
    <Card elevation={1}>
      <ul class="outputs">
        {#each outputs as o (o.kind)}
          <li class="output">
            <span class="file-icon"><FileIcon /></span>
            <span class="output-body">
              <span class="output-label">{o.label}</span>
              <span class="mono">{o.path}</span>
            </span>
          </li>
        {/each}
      </ul>
    </Card>
  {/if}

  {#if revealError}
    <Banner role="status" title="フォルダを開けませんでした">{revealError}</Banner>
  {/if}

  <div class="actions">
    {#if outputs.length > 0}
      <Button onclick={openFolder}><FolderIcon />出力フォルダを開く</Button>
    {/if}
    <Button onclick={() => pipelineState.set({ view: "idle" })}>別のファイルを変換する</Button>
    <Button variant="text" aria-expanded={showLog} aria-controls="step-log" onclick={() => (showLog = !showLog)}>
      ログ詳細
    </Button>
  </div>

  {#if showLog}
    <div id="step-log">
      <Card elevation={0}>
        <StepLog results={pipeline.results} />
      </Card>
    </div>
  {/if}
</div>

<style>
  .result {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-md);
  }

  .heading {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-xs);
    min-width: 0;
  }

  .tray {
    color: var(--color-primary);
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .message {
    font-weight: var(--font-weight-bold);
  }

  .outputs {
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
    padding: 0;
    list-style: none;
  }

  .output {
    display: flex;
    gap: var(--space-sm);
  }

  .file-icon {
    display: inline-flex;
    padding-top: var(--space-2xs);
    color: var(--color-text-muted);
  }

  .output-body {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .output-label {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
  }

  .mono {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    overflow-wrap: anywhere;
  }

  .muted {
    color: var(--color-text-muted);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-xs);
  }
</style>
