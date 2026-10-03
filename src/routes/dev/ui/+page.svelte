<!--
  基本部品の確認用ページ（開発時のみ。/dev/ui）
  enabled / hover / focus（Tab）/ pressed / disabled と、ライト・ダークの見え方を確認する
-->
<script lang="ts">
  import Banner from "$lib/components/ui/Banner.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Card from "$lib/components/ui/Card.svelte";
  import Dialog from "$lib/components/ui/Dialog.svelte";
  import ListItem from "$lib/components/ui/ListItem.svelte";
  import ProgressIndicator from "$lib/components/ui/ProgressIndicator.svelte";
  import StatusChip from "$lib/components/ui/StatusChip.svelte";
  import ArrowDownIcon from "$lib/components/ui/icons/ArrowDownIcon.svelte";
  import ErrorIcon from "$lib/components/ui/icons/ErrorIcon.svelte";
  import FolderIcon from "$lib/components/ui/icons/FolderIcon.svelte";
  import SuccessIcon from "$lib/components/ui/icons/SuccessIcon.svelte";
  import WarningIcon from "$lib/components/ui/icons/WarningIcon.svelte";

  type Theme = "system" | "light" | "dark";
  // ?theme=light / ?theme=dark で初期テーマを指定できる（スクリーンショット確認用）
  const initialTheme = new URLSearchParams(location.search).get("theme");
  let theme = $state<Theme>(initialTheme === "light" || initialTheme === "dark" ? initialTheme : "system");
  let dialogOpen = $state(false);
  let dragged = $state(false);
  let lastAction = $state("（まだ押していません）");
  let demoProgress = $state(0.42);

  $effect(() => {
    if (theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", theme);
    return () => document.documentElement.removeAttribute("data-theme");
  });
</script>

<main class="page">
  <header class="row">
    <h1>基本部品の確認</h1>
    <div class="row">
      {#each ["system", "light", "dark"] as const as t (t)}
        <Button variant={theme === t ? "filled" : "text"} onclick={() => (theme = t)}>{t}</Button>
      {/each}
    </div>
  </header>

  <section>
    <h2>Button</h2>
    <p class="note">hover・押下・Tabでのフォーカスで状態レイヤーが重なることを確認する。最後に押したボタン：{lastAction}</p>
    <div class="row">
      <Button onclick={() => (lastAction = "Filled")}>上書きする</Button>
      <Button variant="text" onclick={() => (lastAction = "Text")}>キャンセル</Button>
      <Button onclick={() => (lastAction = "Filled＋アイコン")}><FolderIcon />出力フォルダを開く</Button>
      <Button disabled>無効（Filled）</Button>
      <Button variant="text" disabled>無効（Text）</Button>
    </div>
  </section>

  <section>
    <h2>Card</h2>
    <div class="grid">
      {#each [0, 1, 2, 3] as const as level (level)}
        <Card elevation={level}>elevation {level}</Card>
      {/each}
      <Card {dragged}>
        <div class="stack">
          <span>dragged: {dragged}</span>
          <Button variant="text" onclick={() => (dragged = !dragged)}>切り替え</Button>
        </div>
      </Card>
      <Card disabled>
        <div class="stack"><ArrowDownIcon size={32} /><span>disabled</span></div>
      </Card>
    </div>
  </section>

  <section>
    <h2>ListItem</h2>
    <Card>
      <ul class="list">
        <ListItem status="running" label="音声結合" progress={demoProgress} />
        <ListItem status="running" label="PDF変換" />
        <ListItem status="success" label="pptx解析" detail="12枚のスライドを解析しました" />
        <ListItem status="warning" label="PDF変換" detail="PDFに変換しました" />
        <ListItem
          status="error"
          label="タイムスタンプ書き出し"
          detail="音声結合が失敗したため実行しませんでした。C:\Users\example\Documents\very\long\path\to\lecture_timestamps.json"
        />
      </ul>
    </Card>
  </section>

  <section>
    <h2>ProgressIndicator</h2>
    <p class="note">OSのアニメーション効果をオフにすると回転が止まり、ラベルが表示される。</p>
    <div class="row">
      <ProgressIndicator size="sm" label="処理中" />
      <ProgressIndicator size="lg" label="pptxを解析しています" />
    </div>
    <h3>確定型（value あり）</h3>
    <p class="note">
      バーの右に割合を表示する。値を変えるとバーが伸び縮みし、アニメーション効果をオフにすると動きなしで値だけ変わる。
      上の ListItem（音声結合）もこの値で表示する。
    </p>
    <div class="stack-wide">
      <ProgressIndicator value={0} label="0%の例" />
      <ProgressIndicator value={0.42} label="42%の例" />
      <ProgressIndicator value={1} label="100%の例" />
      <label class="slider">
        <span>値を変える</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={demoProgress} />
        <ProgressIndicator value={demoProgress} label="操作できる例" />
      </label>
    </div>
  </section>

  <section>
    <h2>Dialog</h2>
    <p class="note">Tabでフォーカスがダイアログ内に閉じ込められ、Escで閉じることを確認する。</p>
    <Button onclick={() => (dialogOpen = true)}>ダイアログを開く</Button>
    <Dialog open={dialogOpen} title="上書きしますか？" onclose={() => (dialogOpen = false)}>
      <p>出力先に同じ名前のファイルがあります。</p>
      <ul class="mono">
        <li>lecture_audio.m4a</li>
        <li>lecture_timestamps.json</li>
      </ul>
      {#snippet actions()}
        <Button variant="text" onclick={() => (dialogOpen = false)}>キャンセル</Button>
        <Button onclick={() => (dialogOpen = false)}>上書きする</Button>
      {/snippet}
    </Dialog>
  </section>

  <section>
    <h2>Banner</h2>
    <div class="stack-wide">
      <Banner role="alert" title="設定ファイルにエラーがあります" items={["ffmpegPath が見つかりません", "sofficePath が見つかりません"]}>
        <span class="mono">C:\Users\example\AppData\Roaming\app.settings.json</span>
        {#snippet actions()}
          <Button variant="text">設定を再読み込み</Button>
        {/snippet}
      </Banner>
      <Banner title="警告" items={["スライド3：音声ファイルのリンクが切れています"]} />
      <Banner>pptx または ppsx ファイルをドロップしてください</Banner>
    </div>
  </section>

  <section>
    <h2>StatusChip</h2>
    <div class="row">
      <StatusChip status="done" />
      <StatusChip status="partial" />
      <StatusChip status="failed" />
    </div>
  </section>

  <section>
    <h2>アイコン</h2>
    <div class="row">
      <SuccessIcon size={24} title="成功" />
      <ErrorIcon size={24} title="失敗" />
      <WarningIcon size={24} title="警告" />
      <ArrowDownIcon size={24} title="矢印" />
      <FolderIcon size={24} title="フォルダ" />
    </div>
  </section>
</main>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-xl);
    max-width: 800px;
    margin: 0 auto;
    padding: var(--space-lg);
  }

  h1 {
    font-size: var(--font-size-xl);
  }

  h2 {
    margin-bottom: var(--space-sm);
    font-size: var(--font-size-lg);
  }

  .note {
    margin-bottom: var(--space-sm);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-sm);
  }

  section .row {
    justify-content: flex-start;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: var(--space-md);
  }

  .stack {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-xs);
  }

  .stack-wide {
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
  }

  .list {
    padding: 0;
  }

  h3 {
    margin: var(--space-md) 0 var(--space-xs);
    font-size: var(--font-size-base);
  }

  .slider {
    display: flex;
    flex-direction: column;
    gap: var(--space-2xs);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .mono {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
  }
</style>
