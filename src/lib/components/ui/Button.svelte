<!--
  Button（M3: Filled Button / Text Button）
  filled＝主要操作、text＝補助操作。高さ40px・横最小64px・shape md
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  let {
    variant = "filled",
    type = "button",
    disabled = false,
    children,
    ...rest
  }: HTMLButtonAttributes & {
    variant?: "filled" | "text";
    children: Snippet;
  } = $props();
</script>

<button {type} {disabled} class="button state-layer shape-md {variant}" {...rest}>
  {@render children()}
</button>

<style>
  .button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-xs);
    min-width: 64px;
    min-height: 40px;
    border: none;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    line-height: var(--line-height-tight);
    cursor: pointer;
    transition: color var(--motion-duration-fast) var(--motion-easing-standard),
      background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }

  .filled {
    padding: 0 var(--space-lg);
    background: var(--color-primary);
    color: var(--color-text-on-primary);
  }

  /* 文字は --color-text（primary 系の文字色は背景に対し 4.5:1 を満たさないため） */
  .text {
    padding: 0 var(--space-sm);
    background: transparent;
    color: var(--color-text);
  }

  .button:disabled {
    cursor: default;
    color: color-mix(in srgb, var(--color-text) calc(var(--opacity-disabled-content) * 100%), transparent);
  }

  .filled:disabled {
    background: color-mix(in srgb, var(--color-text) calc(var(--opacity-disabled-container) * 100%), transparent);
  }
</style>
