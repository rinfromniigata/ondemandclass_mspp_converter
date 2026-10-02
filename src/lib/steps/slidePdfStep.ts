// スライドを PDF に変換する（Rust: run_soffice_convert）。音声結合とは独立して並行実行できる
import { commands, type Commands } from "../tauriCommands";
import type { ActionStep, StepResult } from "./types";

export interface SlidePdfInput {
  inputPath: string;
  outPath: string;
}

export class SlidePdfStep implements ActionStep<SlidePdfInput, string> {
  readonly name = "PDF変換";

  constructor(private readonly cmd: Pick<Commands, "runSofficeConvert"> = commands) {}

  async execute({ inputPath, outPath }: SlidePdfInput): Promise<StepResult & { data?: string }> {
    try {
      const written = await this.cmd.runSofficeConvert(inputPath, outPath);
      return { stepName: this.name, success: true, message: "PDFに変換しました", outputPath: written, data: written };
    } catch (e) {
      return { stepName: this.name, success: false, message: String(e) };
    }
  }
}
