<!-- 警告（三角に感嘆符） -->
<script lang="ts">
  import IconBase from "./IconBase.svelte";

  let { size, title }: { size?: number; title?: string } = $props();
</script>

<IconBase {size} {title}>
  <path d="M12 3.5L21.5 20h-19z" />
  <path d="M12 10v4M12 17v.01" />
</IconBase>
