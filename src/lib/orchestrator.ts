// 4つのStepを順序どおりに実行し、成果物と失敗ステップを集計する
// pptx解析のみ必須先行ステップ。音声結合とPDF変換は並列実行し、片方が失敗してももう片方の成果物は残す
import type { AudioConcatInput, AudioConcatOutput } from "./steps/audioConcatStep";
import type { SlidePdfInput } from "./steps/slidePdfStep";
import type { TimestampJsonInput } from "./steps/timestampJsonStep";
import type { ActionStep, OutputPaths, SlideAudioMap, StepResult } from "./steps/types";

export interface PipelineCallbacks {
  onStart: (stepName: string) => void; // ステップ開始時（Progress Indicator表示用）
  onProgress: (r: StepResult) => void; // ステップ完了時（成功・失敗とも）
  onStepProgress: (stepName: string, ratio: number) => void; // ステップ内の進捗（0〜1）。音声結合だけが通知する
}

export interface RunSummary {
  outputs: Partial<{ audio: string; pdf: string; json: string }>;
  failedSteps: string[];
}

export interface PipelineSteps {
  extract: ActionStep<{ inputPath: string }, SlideAudioMap>;
  audio: ActionStep<AudioConcatInput, AudioConcatOutput>;
  pdf: ActionStep<SlidePdfInput, string>;
  json: ActionStep<TimestampJsonInput, string>;
}

export class PipelineOrchestrator {
  // 各Stepはコンストラクタで注入する（テスト時にモックへ差し替えるため）
  constructor(private readonly steps: PipelineSteps) {}

  async run(inputPath: string, outputs: OutputPaths, cb: PipelineCallbacks): Promise<RunSummary> {
    const { extract, audio, pdf, json } = this.steps;

    cb.onStart(extract.name);
    const extractResult = await extract.execute({ inputPath });
    cb.onProgress(extractResult);
    if (!extractResult.success || !extractResult.data) {
      return { outputs: {}, failedSteps: [extractResult.stepName] };
    }

    // 並列実行し、到着順に onProgress へ通知する
    // 音声結合は進捗を割合で表示するため、最初の通知が届く前から 0 を通知しておく
    cb.onStart(audio.name);
    cb.onStepProgress(audio.name, 0);
    cb.onStart(pdf.name);
    const audioPromise = audio
      .execute({
        inputPath,
        slideAudioMap: extractResult.data,
        outPath: outputs.audio,
        onProgress: (ratio) => cb.onStepProgress(audio.name, ratio),
      })
      .then((r) => {
        cb.onProgress(r);
        return r;
      });
    const pdfPromise = pdf.execute({ inputPath, outPath: outputs.pdf }).then((r) => {
      cb.onProgress(r);
      return r;
    });
    const [audioResult, pdfResult] = await Promise.all([audioPromise, pdfPromise]);

    const summary: RunSummary = { outputs: {}, failedSteps: [] };

    if (audioResult.success) summary.outputs.audio = outputs.audio;
    else summary.failedSteps.push(audioResult.stepName);

    if (pdfResult.success) summary.outputs.pdf = outputs.pdf;
    else summary.failedSteps.push(pdfResult.stepName);

    // JSON は音声結合の成功時のみ書き出す。失敗時は onStart を呼ばずに失敗結果だけ通知する
    let jsonResult: StepResult;
    if (audioResult.success && audioResult.data) {
      cb.onStart(json.name);
      jsonResult = await json.execute({ timestamps: audioResult.data.timestamps, outPath: outputs.json });
    } else {
      jsonResult = { stepName: json.name, success: false, message: "音声結合が失敗したため実行しませんでした" };
    }
    cb.onProgress(jsonResult);

    if (jsonResult.success) summary.outputs.json = outputs.json;
    else summary.failedSteps.push(jsonResult.stepName);

    return summary;
  }
}
