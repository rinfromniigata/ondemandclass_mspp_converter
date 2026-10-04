<!--
  Progress Indicator
  - value なし：不定形（M3: Circular Progress Indicator）。reduced-motion 時は回転を止め、ラベルを文字で表示して代替する
  - value あり：確定型（M3: Linear Progress Indicator）。バーと割合（%）を表示する。親要素の幅いっぱいに広がる
    reduced-motion 時はトークンの duration が0になり、バーは伸びる動きなしで値だけ更新される
-->
<script lang="ts">
  let {
    size = "sm",
    label,
    value,
  }: {
    size?: "sm" | "lg"; // 不定形のみ
    label: string; // 読み上げ用。不定形では reduced-motion 時に画面にも表示する
    value?: number; // 0〜1。指定すると確定型になる
  } = $props();

  const px = $derived(size === "lg" ? 48 : 24);
  const ratio = $derived(value === undefined || Number.isNaN(value) ? 0 : Math.min(Math.max(value, 0), 1));
  const percent = $derived(Math.floor(ratio * 100));
</script>

{#if value === undefined}
  <span class="progress {size}" role="progressbar" aria-label={label}>
    <svg class="spinner" width={px} height={px} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle class="track" cx="12" cy="12" r="9" />
      <circle class="arc" cx="12" cy="12" r="9" pathLength="100" />
    </svg>
    <span class="static-label" aria-hidden="true">{label}</span>
  </span>
{:else}
  <span
    class="linear"
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={percent}
    aria-valuetext="{percent}%"
  >
    <span class="bar" aria-hidden="true">
      <span class="fill" style:transform="scaleX({ratio})"></span>
    </span>
    <span class="percent" aria-hidden="true">{percent}%</span>
  </span>
{/if}

<style>
  .progress {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2xs);
    color: var(--color-primary);
  }

  .spinner {
    animation: spin 1s linear infinite;
  }

  circle {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.5;
  }

  .track {
    stroke: color-mix(in srgb, currentColor 24%, transparent);
  }

  .arc {
    stroke-linecap: round;
    stroke-dasharray: 30 100;
  }

  .static-label {
    display: none;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  /* 確定型 */
  .linear {
    display: flex;
    align-items: center;
    gap: var(--space-xs);
    width: 100%;
    color: var(--color-primary);
  }

  .bar {
    flex: 1;
    height: var(--space-2xs);
    overflow: hidden;
    border-radius: var(--radius-full);
    background: color-mix(in srgb, currentColor 24%, transparent);
  }

  .fill {
    display: block;
    height: 100%;
    background: currentColor;
    transform-origin: left;
    transition: transform var(--motion-duration-fast) var(--motion-easing-standard);
  }

  .percent {
    /* 「100%」が収まる幅に固定し、数字の変化でバーの長さが揺れないようにする（「%」は数字より幅が広いため 5ch） */
    min-width: 5ch;
    font-size: var(--font-size-sm);
    font-variant-numeric: tabular-nums;
    text-align: right;
    color: var(--color-text-muted);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }

    .static-label {
      display: inline;
    }
  }
</style>
