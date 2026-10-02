<!--
  processing 画面。pptx解析中は中央に ProgressIndicator を1つ、以降は処理ログを表示する
  段階表示（「1/4」等）はしない
-->
<script lang="ts">
  import { fileNameOf } from "../outputPaths";
  import type { PipelineState } from "../steps/types";
  import StepLog from "./StepLog.svelte";
  import Card from "./ui/Card.svelte";
  import ProgressIndicator from "./ui/ProgressIndicator.svelte";

  let { pipeline }: { pipeline: Extract<PipelineState, { view: "processing" }> } = $props();

  // pptx解析は必ず最初に単独で実行されるため、完了したステップがない間は解析中
  const extracting = $derived(pipeline.results.length === 0);
  const fileName = $derived(fileNameOf(pipeline.inputPath));
</script>

<div class="processing">
  <header class="header">
    <h1 class="title">変換しています</h1>
    <p class="mono">{fileName}</p>
  </header>

  {#if extracting}
    <div class="center">
      <ProgressIndicator size="lg" label="スライドを解析しています" />
    </div>
  {:else}
    <Card elevation={1}>
      <StepLog results={pipeline.results} running={pipeline.running} />
    </Card>
  {/if}
</div>

<style>
  .processing {
    display: flex;
    flex-direction: column;
    gap: var(--space-lg);
    flex: 1;
  }

  .header {
    display: flex;
    flex-direction: column;
    gap: var(--space-2xs);
  }

  .title {
    font-size: var(--font-size-xl);
    line-height: var(--line-height-tight);
  }

  .mono {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    overflow-wrap: anywhere;
  }

  .center {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
  }
</style>
