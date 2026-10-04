<!--
  idle 画面。pptx / ppsx のドロップを受け付け、設定エラーと受付拒否理由を Banner で表示する
  拡張子・件数の判定は pipelineController が行う
-->
<script lang="ts">
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount } from "svelte";
  import { pipelineController } from "../pipelineController";
  import { reloadSettings, settingsStatus } from "../settings";
  import Banner from "./ui/Banner.svelte";
  import Button from "./ui/Button.svelte";
  import Card from "./ui/Card.svelte";
  import ArrowDownIcon from "./ui/icons/ArrowDownIcon.svelte";

  let { notice }: { notice?: string } = $props();

  let dragged = $state(false);
  let reloading = $state(false);

  const settingsError = $derived($settingsStatus !== null && !$settingsStatus.ok);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    getCurrentWebview()
      .onDragDropEvent(({ payload }) => {
        if (payload.type === "enter" || payload.type === "over") {
          dragged = !settingsError;
        } else if (payload.type === "leave") {
          dragged = false;
        } else if (payload.type === "drop") {
          dragged = false;
          if (!settingsError) void pipelineController.startConversion(payload.paths);
        }
      })
      .then((fn) => {
        // 登録完了前にアンマウントされた場合はすぐに解除する
        if (disposed) fn();
        else unlisten = fn;
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  async function reload() {
    reloading = true;
    try {
      await reloadSettings();
    } finally {
      reloading = false;
    }
  }
</script>

<div class="drop-zone">
  {#if settingsError && $settingsStatus}
    <Banner role="alert" title="設定ファイルにエラーがあります" items={$settingsStatus.errors}>
      {#if $settingsStatus.settingsPath}
        <span class="mono">{$settingsStatus.settingsPath}</span>
      {/if}
      {#snippet actions()}
        <Button variant="text" disabled={reloading} onclick={reload}>設定を再読み込み</Button>
      {/snippet}
    </Banner>
  {/if}

  {#if notice}
    <Banner role="status">{notice}</Banner>
  {/if}

  <Card elevation={1} {dragged} disabled={settingsError} class="target">
    <div class="prompt">
      <span class="arrow"><ArrowDownIcon size={48} /></span>
      <p>ここに pptx / ppsx をドロップ</p>
    </div>
  </Card>
</div>

<style>
  .drop-zone {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
    flex: 1;
  }

  .drop-zone :global(.target) {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    min-height: 200px;
  }

  .prompt {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-md);
    text-align: center;
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-medium);
  }

  .arrow {
    color: var(--color-primary-strong);
  }

  .mono {
    font-family: var(--font-family-mono);
  }
</style>
