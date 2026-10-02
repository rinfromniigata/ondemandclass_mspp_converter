<!--
  List item（M3: List item、leading icon 付き）。StepResult 1件＝1行
  状態を色だけで示さず、アイコン＋状態ラベルの文字を必ず併記する。<ul> の中に置く
-->
<script lang="ts">
  import ProgressIndicator from "./ProgressIndicator.svelte";
  import ErrorIcon from "./icons/ErrorIcon.svelte";
  import SuccessIcon from "./icons/SuccessIcon.svelte";

  type Status = "running" | "success" | "warning" | "error";

  let {
    status,
    label,
    detail,
  }: {
    status: Status;
    label: string; // ステップ名
    detail?: string; // メッセージ等の補足
  } = $props();

  const STATUS_LABELS: Record<Status, string> = {
    running: "処理中",
    success: "完了",
    warning: "完了（警告あり）",
    error: "失敗",
  };
</script>

<li class="list-item {status}">
  <span class="leading">
    {#if status === "running"}
      <ProgressIndicator size="sm" label="{label}を処理中" />
    {:else if status === "success" || status === "warning"}
      <!-- 完了（警告あり）も成功アイコン。警告であることは状態ラベルで示す（スペック8章） -->
      <SuccessIcon size={24} />
    {:else}
      <ErrorIcon size={24} />
    {/if}
  </span>
  <span class="body">
    <span class="headline">
      <span class="label">{label}</span>
      <span class="status-label">{STATUS_LABELS[status]}</span>
    </span>
    {#if detail}
      <span class="detail">{detail}</span>
    {/if}
  </span>
</li>

<style>
  .list-item {
    display: flex;
    align-items: flex-start;
    gap: var(--space-md);
    min-height: 40px;
    padding: var(--space-xs) 0;
    list-style: none;
  }

  .leading {
    display: inline-flex;
    padding-top: var(--space-2xs);
  }

  /* 処理中の ProgressIndicator は reduced-motion 時に文字を出すが、行内では状態ラベルがあるため隠す */
  .leading :global(.static-label) {
    display: none;
  }

  .success .leading,
  .warning .leading {
    color: var(--color-success);
  }

  .error .leading {
    color: var(--color-error);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-2xs);
    min-width: 0;
  }

  .headline {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-xs);
  }

  .label {
    font-weight: var(--font-weight-medium);
  }

  .status-label {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .detail {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    overflow-wrap: anywhere;
  }
</style>
