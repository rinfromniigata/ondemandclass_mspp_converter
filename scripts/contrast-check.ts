// tokens.css からライト・ダーク両方の色を読み取り、実際に使う「文字色×背景色」のコントラスト比を一覧出力する（imple 6.3）
// 4.5:1 未満の組み合わせがあれば終了コード1で終わる
// 実行: bun scripts/contrast-check.ts
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const TOKENS_PATH = fileURLToPath(new URL("../src/lib/styles/tokens.css", import.meta.url));
const TEXT_MIN = 4.5; // WCAG AA（本文）
const NON_TEXT_MIN = 3; // WCAG AA（文字以外の部品）。参考表示のみで終了コードには影響させない
// Banner の背景（Banner.svelte）：color-mix(in srgb, var(--color-error) 12%, var(--color-surface))
const BANNER_ERROR_RATIO = 0.12;

type Rgb = [number, number, number];
type Palette = Record<string, Rgb>;

/** `{` に対応する `}` までの中身を返す */
function blockBody(css: string, openIndex: number): string {
  let depth = 0;
  for (let i = openIndex; i < css.length; i++) {
    if (css[i] === "{") depth++;
    else if (css[i] === "}" && --depth === 0) return css.slice(openIndex + 1, i);
  }
  throw new Error("tokens.css の波かっこが閉じていません");
}

/** セレクタに続くブロックから `--color-*: #rrggbb` を取り出す */
function readColors(css: string, selector: string, from = 0): Palette {
  const at = css.indexOf(selector, from);
  if (at < 0) throw new Error(`tokens.css に ${selector} が見つかりません`);
  const body = blockBody(css, css.indexOf("{", at + selector.length - 1));
  const palette: Palette = {};
  for (const m of body.matchAll(/(--color-[\w-]+)\s*:\s*#([0-9a-fA-F]{6})\s*;/g)) {
    const hex = m[2];
    palette[m[1]] = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16)) as Rgb;
  }
  return palette;
}

function relativeLuminance([r, g, b]: Rgb): number {
  const lin = (c: number) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** color-mix(in srgb, a p, b) と同じ混合（sRGB の値をそのまま線形補間） */
function mixSrgb(a: Rgb, b: Rgb, p: number): Rgb {
  return a.map((c, i) => Math.round(c * p + b[i] * (1 - p))) as Rgb;
}

const toHex = (c: Rgb) => "#" + c.map((v) => v.toString(16).padStart(2, "0")).join("");

const css = readFileSync(TOKENS_PATH, "utf8");
const light = readColors(css, ":root {");
const darkOverrides = readColors(css, ':root[data-theme="dark"] {');
const darkMedia = readColors(css, ':root:not([data-theme="light"]) {', css.indexOf("@media (prefers-color-scheme: dark)"));

// OS設定に追従するダークと、手動指定のダークが同じ値であることを確かめる
const mismatched = Object.keys({ ...darkOverrides, ...darkMedia }).filter(
  (k) => toHex(darkOverrides[k] ?? [0, 0, 0]) !== toHex(darkMedia[k] ?? [-1, -1, -1]),
);
if (mismatched.length > 0) {
  console.error(`ダークモードの2つの定義で値が異なります: ${mismatched.join(", ")}`);
  process.exit(1);
}

const themes: Record<string, Palette> = { light, dark: { ...light, ...darkOverrides } };

// 背景の名前 → 色。Banner背景は混合色として計算する
function background(p: Palette, name: string): Rgb {
  if (name === "Banner背景") return mixSrgb(p["--color-error"], p["--color-surface"], BANNER_ERROR_RATIO);
  return p[name];
}

const TEXT_PAIRS: [string, string, string][] = [
  ...["--color-text", "--color-text-muted"].flatMap((fg) =>
    ["--color-bg", "--color-surface", "--color-surface-sunken", "Banner背景"].map(
      (bg) => [fg, bg, "本文・補足文"] as [string, string, string],
    ),
  ),
  ["--color-text-on-primary", "--color-primary", "Filled Button"],
  ["--color-text-on-primary", "--color-primary-strong", "primary-strong 上の文字"],
  ["--color-text-on-secondary", "--color-secondary", "secondary 上の文字"],
];

const NON_TEXT_PAIRS: [string, string, string][] = [
  ["--color-primary-strong", "--color-bg", "フォーカスリング（背景上）"],
  ["--color-primary-strong", "--color-surface", "フォーカスリング・DropZoneの矢印（Card上）"],
  ["--color-primary", "--color-surface", "ProgressIndicator"],
  ["--color-success", "--color-surface", "成功アイコン"],
  ["--color-warning", "--color-surface", "警告アイコン"],
  ["--color-error", "--color-surface", "失敗アイコン"],
  ["--color-error", "Banner背景", "Bannerのアイコン・帯"],
];

const pad = (s: string, n: number) => s + " ".repeat(Math.max(0, n - [...s].reduce((w, ch) => w + (ch.charCodeAt(0) > 0xff ? 2 : 1), 0)));
let failures = 0;

for (const [theme, p] of Object.entries(themes)) {
  console.log(`\n=== ${theme} ===`);
  console.log("[文字] 基準 4.5:1");
  for (const [fg, bg, use] of TEXT_PAIRS) {
    const ratio = contrast(p[fg], background(p, bg));
    const ok = ratio >= TEXT_MIN;
    if (!ok) failures++;
    console.log(`  ${ok ? "OK  " : "NG  "}${ratio.toFixed(2).padStart(5)}:1  ${pad(fg, 26)} × ${pad(`${bg} (${toHex(background(p, bg))})`, 34)} ${use}`);
  }
  console.log("[参考：文字以外] 基準 3:1（判定に含めない）");
  for (const [fg, bg, use] of NON_TEXT_PAIRS) {
    const ratio = contrast(p[fg], background(p, bg));
    console.log(`  ${ratio >= NON_TEXT_MIN ? "ok  " : "低  "}${ratio.toFixed(2).padStart(5)}:1  ${pad(fg, 26)} × ${pad(bg, 34)} ${use}`);
  }
}

console.log(failures === 0 ? "\nすべての文字色×背景色が 4.5:1 以上です" : `\n4.5:1 未満の組み合わせが ${failures} 件あります`);
process.exit(failures === 0 ? 0 : 1);
