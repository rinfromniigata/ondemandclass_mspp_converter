<!--
  Status Chip（M3: Assist Chip）。ResultView 冒頭の状態表示。shape full
  状態はアイコン＋ラベルの文字で示す
-->
<script lang="ts">
  import ErrorIcon from "./icons/ErrorIcon.svelte";
  import SuccessIcon from "./icons/SuccessIcon.svelte";
  import WarningIcon from "./icons/WarningIcon.svelte";

  type Status = "done" | "partial" | "failed";

  let { status }: { status: Status } = $props();

  const LABELS: Record<Status, string> = {
    done: "完了",
    partial: "一部エラー",
    failed: "失敗",
  };
</script>

<span class="chip shape-full {status}">
  <span class="icon">
    {#if status === "done"}
      <SuccessIcon size={18} />
    {:else if status === "partial"}
      <WarningIcon size={18} />
    {:else}
      <ErrorIcon size={18} />
    {/if}
  </span>
  <span class="label">{LABELS[status]}</span>
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-xs);
    min-height: 32px;
    padding: 0 var(--space-sm);
    border: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
  }

  .icon {
    display: inline-flex;
  }

  .done .icon {
    color: var(--color-success);
  }

  .partial .icon {
    color: var(--color-warning);
  }

  .failed .icon {
    color: var(--color-error);
  }
</style>
