<!--
  Progress Indicator（M3: Circular Progress Indicator、不定形）
  reduced-motion 時は回転を止め、ラベルを文字で表示して代替する
-->
<script lang="ts">
  let {
    size = "sm",
    label,
  }: {
    size?: "sm" | "lg";
    label: string; // 読み上げ用。reduced-motion 時は画面にも表示する
  } = $props();

  const px = $derived(size === "lg" ? 48 : 24);
</script>

<span class="progress {size}" role="progressbar" aria-label={label}>
  <svg class="spinner" width={px} height={px} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <circle class="track" cx="12" cy="12" r="9" />
    <circle class="arc" cx="12" cy="12" r="9" pathLength="100" />
  </svg>
  <span class="static-label" aria-hidden="true">{label}</span>
</span>

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
