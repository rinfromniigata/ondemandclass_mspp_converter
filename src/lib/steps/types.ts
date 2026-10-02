// パイプラインで受け渡すデータの型。Rust側の serde 型（camelCase）と一致させる

export interface SlideAudioEntry {
  slideIndex: number; // 1始まり、表示順
  slideXmlPath: string; // 例: "ppt/slides/slide3.xml"
  audioMediaPaths: string[]; // 例: ["ppt/media/media2.m4a"]（再生順。リンク切れは含めない）
  hasAudio: boolean; // audioMediaPaths が1件以上なら true
  linkBroken: boolean; // 外部リンク参照・実体なしの音声が1件以上あれば true
  advanceSec: number | null; // p:transition の advTm（自動切り替え時間、秒）。未設定なら null
}

export interface SlideAudioMap {
  slides: SlideAudioEntry[]; // 表示順
  warnings: string[]; // リンク切れ・動画ナレーション検出等（スライド番号を含む文言）
}

// 結合音声を構成する区間。スライドの表示順に並ぶ
export type AudioSegment =
  | { slideIndex: number; kind: "media"; mediaPath: string } // pptx内のメディアパス
  | { slideIndex: number; kind: "silence"; durationSec: number };

export interface SlideTimestampEntry {
  slide: number; // slideIndex
  startSec: number; // 結合音声内での開始秒
  endSec: number; // 結合音声内での終了秒（skip時の無音スライドは startSec と同値）
}

export interface OutputPaths {
  dir: string; // 入力ファイルのディレクトリ
  basename: string; // 拡張子を除いた入力ファイル名
  audio: string; // <dir>/<basename>_audio.m4a
  pdf: string; // <dir>/<basename>_slides.pdf
  json: string; // <dir>/<basename>_timestamps.json
}

export interface StepResult {
  stepName: string;
  success: boolean;
  message: string;
  warnings?: string[]; // 成功扱いだが利用者に知らせるべき事項（リンク切れ等）。Bannerで表示する
  outputPath?: string;
}

export interface ActionStep<TInput, TOutput> {
  readonly name: string;
  execute(input: TInput): Promise<StepResult & { data?: TOutput }>;
}

// パイプライン全体の状態。Wizard.svelte はこれだけを見て表示を切り替える
export type PipelineState =
  | { view: "idle"; notice?: string } // notice: 直前の受付拒否理由（拡張子不正・複数ファイル等）
  | { view: "processing"; inputPath: string; running: string[]; results: StepResult[] } // running: 実行中ステップの name
  | { view: "done"; inputPath: string; results: StepResult[]; outputs: { audio: string; pdf: string; json: string } }
  | {
      view: "error";
      inputPath: string;
      results: StepResult[];
      outputs: Partial<{ audio: string; pdf: string; json: string }>;
      failedSteps: string[];
    };
