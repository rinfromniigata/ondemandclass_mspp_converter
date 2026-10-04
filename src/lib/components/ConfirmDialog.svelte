<!--
  confirmDialog ストアの要求を Dialog で表示する。Esc と「キャンセル」は同じ扱い（false で解決）
-->
<script lang="ts">
  import { pendingConfirm } from "../confirmDialog";
  import Button from "./ui/Button.svelte";
  import Dialog from "./ui/Dialog.svelte";
</script>

<Dialog open={$pendingConfirm !== null} title={$pendingConfirm?.title ?? ""} onclose={() => $pendingConfirm?.resolve(false)}>
  {#if $pendingConfirm}
    <p>{$pendingConfirm.message}</p>
    {#if $pendingConfirm.details && $pendingConfirm.details.length > 0}
      <ul class="details">
        {#each $pendingConfirm.details as d, i (i)}
          <li>{d}</li>
        {/each}
      </ul>
    {/if}
  {/if}
  {#snippet actions()}
    <Button variant="text" onclick={() => $pendingConfirm?.resolve(false)}>{$pendingConfirm?.cancelLabel}</Button>
    <Button onclick={() => $pendingConfirm?.resolve(true)}>{$pendingConfirm?.confirmLabel}</Button>
  {/snippet}
</Dialog>

<style>
  .details {
    padding-left: var(--space-md);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    overflow-wrap: anywhere;
  }
</style>
