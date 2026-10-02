<!--
  Card / Surface container（M3: Elevated Card）
  shape lg・背景 --color-surface。dragged はドラッグオーバー中の表示（elevation 2 ＋ dragged 状態レイヤー）
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  let {
    elevation = 1,
    dragged = false,
    disabled = false,
    class: className = "",
    children,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    elevation?: 0 | 1 | 2 | 3;
    dragged?: boolean;
    disabled?: boolean;
    children: Snippet;
  } = $props();

  // ドラッグオーバー中は少なくとも elevation 2 に浮かせる
  const level = $derived(dragged ? Math.max(elevation, 2) : elevation);
</script>

<div
  class="card shape-lg elevation-{level} {className}"
  class:state-layer={dragged}
  class:dragged
  class:disabled
  aria-disabled={disabled ? "true" : undefined}
  {...rest}
>
  {@render children()}
</div>

<style>
  .card {
    padding: var(--space-lg);
    background: var(--color-surface);
    color: var(--color-text);
    transition: box-shadow var(--motion-duration-base) var(--motion-easing-standard);
  }

  .disabled > :global(*) {
    opacity: var(--opacity-disabled-content);
  }
</style>
