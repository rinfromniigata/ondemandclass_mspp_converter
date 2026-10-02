<!--
  処理ログ。完了したステップを到着順に並べ、実行中のステップをその後ろに並べる
  ProcessingView と ResultView（ログ詳細）で共用する
-->
<script lang="ts">
  import ListItem from "./ui/ListItem.svelte";
  import type { StepResult } from "../steps/types";

  let { results, running = [] }: { results: StepResult[]; running?: string[] } = $props();

  function statusOf(r: StepResult): "success" | "warning" | "error" {
    if (!r.success) return "error";
    return r.warnings && r.warnings.length > 0 ? "warning" : "success";
  }
</script>

<ul class="log">
  {#each results as r, i (i)}
    <ListItem status={statusOf(r)} label={r.stepName} detail={r.message} />
  {/each}
  {#each running as name (name)}
    <ListItem status="running" label={name} />
  {/each}
</ul>

<style>
  .log {
    display: flex;
    flex-direction: column;
    padding: 0;
  }
</style>
