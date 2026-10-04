<script lang="ts">
  // tokens.css → base.css の順に読み込む（base.css がトークンの一部を上書きするため）
  import "$lib/styles/tokens.css";
  import "$lib/styles/base.css";

  let { children } = $props();
</script>

{@render children()}
