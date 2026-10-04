<!-- 下向き矢印（ドロップを促すモチーフ） -->
<script lang="ts">
  import IconBase from "./IconBase.svelte";

  let { size, title }: { size?: number; title?: string } = $props();
</script>

<IconBase {size} {title}>
  <path d="M12 4v15M6 13l6 6 6-6" />
</IconBase>
