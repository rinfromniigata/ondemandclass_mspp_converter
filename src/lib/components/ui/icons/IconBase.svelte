<!-- インラインSVGアイコンの共通枠。色は currentColor を使い、呼び出し側の color に従う -->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    size = 20,
    title,
    children,
  }: {
    size?: number;
    title?: string; // 指定時のみ読み上げ対象にする。省略時は装飾扱い
    children: Snippet;
  } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={title ? "img" : undefined}
  aria-label={title}
  aria-hidden={title ? undefined : "true"}
  focusable="false"
>
  {@render children()}
</svg>
