// pptx/ppsx から各スライドの音声対応表を取り出す（Rust: extract_pptx の薄いラッパー）
import { commands, type Commands } from "../tauriCommands";
import type { ActionStep, SlideAudioMap, StepResult } from "./types";

export class ExtractPptxStep implements ActionStep<{ inputPath: string }, SlideAudioMap> {
  readonly name = "pptx解析";

  constructor(private readonly cmd: Pick<Commands, "extractPptx"> = commands) {}

  async execute({ inputPath }: { inputPath: string }): Promise<StepResult & { data?: SlideAudioMap }> {
    try {
      const map = await this.cmd.extractPptx(inputPath);
      return {
        stepName: this.name,
        success: true,
        message: `${map.slides.length}枚のスライドを解析しました`,
        warnings: map.warnings.length > 0 ? map.warnings : undefined,
        data: map,
      };
    } catch (e) {
      return { stepName: this.name, success: false, message: String(e) };
    }
  }
}
