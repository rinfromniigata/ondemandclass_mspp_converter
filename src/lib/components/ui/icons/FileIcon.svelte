<!-- ファイル（成果物一覧の種別表示） -->
<script lang="ts">
  import IconBase from "./IconBase.svelte";

  let { size, title }: { size?: number; title?: string } = $props();
</script>

<IconBase {size} {title}>
  <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
  <path d="M14 3v5h5" />
</IconBase>
