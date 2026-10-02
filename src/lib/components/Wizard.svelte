<!--
  pipelineState.view に応じて画面を出し分けるだけ（ロジックを持たない）。done と error はどちらも ResultView
  画面の切り替えは不透明度の変化のみ
-->
<script lang="ts">
  import { pipelineState } from "../pipelineStore";
  import DropZone from "./DropZone.svelte";
  import ProcessingView from "./ProcessingView.svelte";
  import ResultView from "./ResultView.svelte";
</script>

{#key $pipelineState.view}
  <div class="screen">
    {#if $pipelineState.view === "idle"}
      <DropZone notice={$pipelineState.notice} />
    {:else if $pipelineState.view === "processing"}
      <ProcessingView pipeline={$pipelineState} />
    {:else}
      <ResultView pipeline={$pipelineState} />
    {/if}
  </div>
{/key}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    flex: 1;
    animation: fade-in var(--motion-duration-base) var(--motion-easing-standard);
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
</style>
