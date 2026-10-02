<!-- 成功（丸にチェック） -->
<script lang="ts">
  import IconBase from "./IconBase.svelte";

  let { size, title }: { size?: number; title?: string } = $props();
</script>

<IconBase {size} {title}>
  <circle cx="12" cy="12" r="9" />
  <path d="M8 12.5l2.5 2.5L16 9.5" />
</IconBase>
