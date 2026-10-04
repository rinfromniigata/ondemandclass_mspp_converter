<!--
  Dialog（M3: Basic Dialog）。elevation 3・shape lg
  <dialog> と showModal() を使い、フォーカスの閉じ込めはブラウザ標準に任せる。
  Esc は onclose を呼ぶだけで、閉じるかどうかは呼び出し側が open で決める
-->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    open,
    title,
    onclose,
    children,
    actions,
  }: {
    open: boolean;
    title: string;
    onclose: () => void; // Esc で呼ばれる（「キャンセル」と同じ扱いにする）
    children: Snippet;
    actions: Snippet;
  } = $props();

  const titleId = $props.id();
  let dialog: HTMLDialogElement;

  $effect(() => {
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  });

  function handleCancel(e: Event) {
    // ブラウザに閉じさせず、open の制御を呼び出し側に残す
    e.preventDefault();
    onclose();
  }
</script>

<dialog bind:this={dialog} class="dialog elevation-3 shape-lg" aria-labelledby={titleId} oncancel={handleCancel}>
  <h2 id={titleId} class="title">{title}</h2>
  <div class="content">
    {@render children()}
  </div>
  <div class="actions">
    {@render actions()}
  </div>
</dialog>

<style>
  .dialog {
    width: min(560px, calc(100vw - 2 * var(--space-lg)));
    max-height: calc(100vh - 2 * var(--space-lg));
    /* base.css のリセット（margin: 0）で失われる中央寄せを戻す */
    margin: auto;
    padding: var(--space-lg);
    border: none;
    background: var(--color-surface);
    color: var(--color-text);
    overflow: auto;
  }

  .dialog[open] {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
    animation: fade-in var(--motion-duration-base) var(--motion-easing-standard);
  }

  .dialog::backdrop {
    background: color-mix(in srgb, var(--color-text) 8%, transparent);
  }

  .title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-bold);
    line-height: var(--line-height-tight);
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-xs);
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
</style>
