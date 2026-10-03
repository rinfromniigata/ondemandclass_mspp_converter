// Rustコマンドの型付き invoke ラッパー。Rust の呼び出しはすべてここを経由する
// Tauri v2 は Rust の snake_case 引数を JS 側 camelCase で受け取るため、引数名は camelCase で渡す
import { Channel, invoke } from "@tauri-apps/api/core";
import type { AudioSegment, SlideAudioMap, SlideTimestampEntry } from "./steps/types";

export type SilentSlideHandling = "insert_silence" | "skip";

// app.settings.json の内容（Rust: settings::AppSettings）
export interface AppSettings {
  ffmpegPath: string;
  ffprobePath: string;
  sofficePath: string;
  silentSlideHandling: SilentSlideHandling;
  silentSlideDefaultSec: number;
  audioReencodeOnMismatch: boolean;
}

// load_and_validate_settings の結果。settings は ok のときだけ非 null
export interface SettingsStatus {
  ok: boolean;
  settingsPath: string;
  settings: AppSettings | null;
  errors: string[];
}

export interface ConcatResult {
  timestamps: SlideTimestampEntry[];
  reencoded: boolean; // 再エンコード方式で結合した場合は true
}

export interface FfmpegConcatArgs {
  inputPath: string;
  slideIndices: number[];
  segments: AudioSegment[];
  outPath: string;
  reencodeOnMismatch: boolean;
}

export const commands = {
  loadAndValidateSettings: () => invoke<SettingsStatus>("load_and_validate_settings"),
  extractPptx: (inputPath: string) => invoke<SlideAudioMap>("extract_pptx", { inputPath }),
  // 進捗（0〜1）は Channel で届く。Rust 側の引数は必須のため、onProgress がなくても Channel は渡す
  runFfmpegConcat: (a: FfmpegConcatArgs, onProgress?: (ratio: number) => void) => {
    const channel = new Channel<number>((ratio) => onProgress?.(ratio));
    return invoke<ConcatResult>("run_ffmpeg_concat", { ...a, onProgress: channel });
  },
  runSofficeConvert: (inputPath: string, outPath: string) =>
    invoke<string>("run_soffice_convert", { inputPath, outPath }),
  checkOutputsExist: (paths: string[]) => invoke<string[]>("check_outputs_exist", { paths }),
  writeTimestampsJson: (outPath: string, entries: SlideTimestampEntry[]) =>
    invoke<string>("write_timestamps_json", { outPath, entries }),
};

export type Commands = typeof commands;
