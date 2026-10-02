<!--
  Banner（M3: Banner）。設定エラー・ステップ警告・受付拒否理由などの非致命的エラー
  文字は --color-text（--color-error の文字は 4.5:1 を満たさないため）。アイコンと左端4pxの帯が --color-error
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import ErrorIcon from "./icons/ErrorIcon.svelte";

  let {
    tone = "error",
    role = "status",
    title,
    items,
    children,
    actions,
  }: {
    tone?: "error";
    role?: "alert" | "status"; // alert＝設定エラー、status＝警告・受付拒否理由
    title?: string;
    items?: string[];
    children?: Snippet;
    actions?: Snippet;
  } = $props();
</script>

<div class="banner shape-md {tone}" {role}>
  <span class="icon">
    <ErrorIcon size={24} />
  </span>
  <div class="body">
    {#if title}
      <p class="title">{title}</p>
    {/if}
    {#if items && items.length > 0}
      <ul class="items">
        {#each items as item, i (i)}
          <li>{item}</li>
        {/each}
      </ul>
    {/if}
    {#if children}
      <div class="extra">{@render children()}</div>
    {/if}
    {#if actions}
      <div class="actions">{@render actions()}</div>
    {/if}
  </div>
</div>

<style>
  .banner {
    display: flex;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-md);
    color: var(--color-text);
  }

  .error {
    border-left: 4px solid var(--color-error);
    background: color-mix(in srgb, var(--color-error) 12%, var(--color-surface));
  }

  .error .icon {
    color: var(--color-error);
  }

  .icon {
    display: inline-flex;
    padding-top: var(--space-2xs);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-2xs);
    min-width: 0;
    font-size: var(--font-size-sm);
  }

  .title {
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-medium);
  }

  .items {
    padding-left: var(--space-md);
    overflow-wrap: anywhere;
  }

  .extra {
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-xs);
    margin-left: calc(-1 * var(--space-sm));
  }
</style>
