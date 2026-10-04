<script lang="ts">
  import { onMount } from "svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import Wizard from "$lib/components/Wizard.svelte";
  import { reloadSettings } from "$lib/settings";

  onMount(() => {
    void reloadSettings();
  });
</script>

<main class="app">
  <Wizard />
</main>
<ConfirmDialog />

<style>
  .app {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    max-width: 720px;
    margin: 0 auto;
    padding: var(--space-lg);
  }
</style>
