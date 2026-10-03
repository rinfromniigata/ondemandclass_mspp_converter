<!--
  List item（M3: List item、leading icon 付き）。StepResult 1件＝1行
  状態を色だけで示さず、アイコン＋状態ラベルの文字を必ず併記する。<ul> の中に置く
  処理中で progress があれば、leading は空けて、2行目に確定型の ProgressIndicator（バー＋割合）を出す
  （動くものをバー1つにするため、回転する表示は出さない）
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
    progress,
  }: {
    status: Status;
    label: string; // ステップ名
    detail?: string; // メッセージ等の補足
    progress?: number; // 処理中の進捗（0〜1）。status が running のときだけ使う
  } = $props();

  const determinate = $derived(status === "running" && progress !== undefined);

  const STATUS_LABELS: Record<Status, string> = {
    running: "処理中",
    success: "完了",
    warning: "完了（警告あり）",
    error: "失敗",
  };
</script>

<li class="list-item {status}">
  <span class="leading">
    {#if determinate}
      <!-- 列をそろえるため幅だけ保つ。進み具合は2行目のバーで示す -->
    {:else if status === "running"}
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
    {#if determinate}
      <ProgressIndicator value={progress} label="{label}の進捗" />
    {/if}
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
    flex-shrink: 0;
    /* アイコン・不定形の ProgressIndicator と同じ24px。空のときも列をそろえる */
    min-width: var(--space-lg);
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
    flex: 1;
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
