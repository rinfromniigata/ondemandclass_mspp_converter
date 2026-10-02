<!-- 失敗（丸にバツ） -->
<script lang="ts">
  import IconBase from "./IconBase.svelte";

  let { size, title }: { size?: number; title?: string } = $props();
</script>

<IconBase {size} {title}>
  <circle cx="12" cy="12" r="9" />
  <path d="M9 9l6 6M15 9l-6 6" />
</IconBase>
