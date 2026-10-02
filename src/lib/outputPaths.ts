// 入力パスから出力3ファイルのパスを決める。出力先の決定ロジックはここに集約する
// （現行は入力と同じディレクトリ固定。スペック14章）
import type { OutputPaths } from "./steps/types";

const SUPPORTED_EXTENSION = /\.(pptx|ppsx)$/i;

/** 拡張子が .pptx / .ppsx（大文字小文字無視）なら true */
export function isSupportedInput(path: string): boolean {
  return SUPPORTED_EXTENSION.test(path);
}

/** パス末尾のファイル名（`\` と `/` の両方を区切りとして扱う） */
export function fileNameOf(path: string): string {
  return path.slice(Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/")) + 1);
}

/** `\` と `/` の両方を区切りとして扱い、出力先は入力で使われていた区切りで組み立てる */
export function resolveOutputPaths(inputPath: string): OutputPaths {
  const sepIndex = Math.max(inputPath.lastIndexOf("\\"), inputPath.lastIndexOf("/"));
  const dir = sepIndex >= 0 ? inputPath.slice(0, sepIndex) : "";
  const sep = sepIndex >= 0 ? inputPath[sepIndex] : "";
  const basename = inputPath.slice(sepIndex + 1).replace(SUPPORTED_EXTENSION, "");
  const prefix = dir + sep + basename;

  return {
    dir,
    basename,
    audio: `${prefix}_audio.m4a`,
    pdf: `${prefix}_slides.pdf`,
    json: `${prefix}_timestamps.json`,
  };
}
