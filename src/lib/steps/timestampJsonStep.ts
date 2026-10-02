// 音声結合で得た各スライドの区間を JSON に書き出す（Rust: write_timestamps_json）
import { commands, type Commands } from "../tauriCommands";
import type { ActionStep, SlideTimestampEntry, StepResult } from "./types";

export interface TimestampJsonInput {
  timestamps: SlideTimestampEntry[];
  outPath: string;
}

export class TimestampJsonStep implements ActionStep<TimestampJsonInput, string> {
  readonly name = "タイムスタンプ書き出し";

  constructor(private readonly cmd: Pick<Commands, "writeTimestampsJson"> = commands) {}

  async execute({ timestamps, outPath }: TimestampJsonInput): Promise<StepResult & { data?: string }> {
    try {
      const written = await this.cmd.writeTimestampsJson(outPath, timestamps);
      return {
        stepName: this.name,
        success: true,
        message: "タイムスタンプを書き出しました",
        outputPath: written,
        data: written,
      };
    } catch (e) {
      return { stepName: this.name, success: false, message: String(e) };
    }
  }
}
