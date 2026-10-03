# 仕様変更サマリー（change）: 音声結合ステップの進捗表示（プログレスバーと割合）

- 状態: **承認済み**（261003、チャットでの承認）
- 反映先: `roadmap/specs/ondemandclass_mspp_converter_SPEC_v3.md`（新版）、`roadmap/planning_dialog/261003120124_imple_audio_progress.md`・`261003120125_tasks_audio_progress.md`・`261003120126_walk_audio_progress.md`
- 反映時の補足（imple作成時に詰めた点）
  - 音声結合の行を最初から 0% で表示するため、Orchestrator が開始直後に 0 を通知する（画面側はステップ名で分岐しない）
  - 処理中の音声結合の行は、回転表示を出さずバー1つだけを動かす（leading 欄は空）
  - Phase 7 の末尾に T7-11（リリースビルドの作り直し）と W7-10（リリースビルドでの表示確認）を追加した。T8-1 は T7-11 に依存する
- デザインアセット（`assets/design/` の tokens・brand・アイコン原本）: **変更なし**

---

## 1. 背景と要望

- 変換中の画面（ProcessingView）は、どのステップも不定形の Circular Progress Indicator（くるくる回る表示）だけを出しており、どこまで進んだかがわからない
- ユーザーの指示（261003）: **全体の進捗は出さない。比較的時間のかかる音声結合（AudioConcatStep）だけ、プログレスバーと進捗の割合（%）を表示する**
- ほかのステップ（pptx解析・PDF変換・JSON書き出し）は今のまま、不定形の表示とする
  - PDF変換は soffice が進捗を出さないため、実際の割合を取れない
  - pptx解析とJSON書き出しは短時間で終わる

## 2. 確定事項・ブランド方針との関係

- スペック3章の基本部品「4. Progress Indicator」（M3対応: Circular、不定形）は確定事項のため、スペックを v3 に改訂する
  - 基本部品は7種のまま増やさない。Progress Indicator に「確定型（M3: Linear Progress Indicator）」のバリエーションを加える
- brand.md の「避けたいもの: 多段階に見えるステップ表現（『1/4』のような段階表示やステッパー）」は**引き続き守る**
  - 表示するのは1つのステップの中の割合（%）とバーだけ。「ステップ 2/4」「区間 3/12」のような件数・段階は出さない
  - 1ステップ内の連続した割合なので、ステッパーには当たらないと判断する
- トークンの値は変えない。バーの色は `--color-primary`、トラックは既存の Circular と同じ `currentColor` の薄い混合、高さ・角丸・モーションは既存トークンを使う

## 3. 進捗の算出方法（音声結合）

`concat_audio` の処理を「準備」と「結合」の2段階に分け、両方を合わせて 0〜100% にする。

1. **準備**: 区間ごとの作業（pptxからの書き出しと ffprobe、無音の生成、再エンコード時の WAV 正規化）を、区間を1つ終えるたびに進める
2. **結合**: 最後の ffmpeg 実行に `-progress pipe:1` を付け、stdout の `out_time_us` ÷ 総尺（準備段階でわかった各区間の長さの合計）で進める
- 2段階の重み（例: 準備 50%・結合 50%）は、実サンプルで copy 結合と再エンコード結合の所要時間を測ってから imple で決める
- 通知は 1% 以上変わったときだけ送り、送りすぎを防ぐ。割合は単調に増やす（戻さない）
- copy 結合が失敗して再エンコードで再試行する場合は、再試行の準備・結合をそのまま続きとして扱わず、割合を戻さないように再試行分を残りの範囲に割り当てる（具体的な割り振りは imple で定める）

## 4. 変更内容

### 4.1 スペックシート（v2 → v3）
- 改訂履歴に v3 を追加
- 3章「4. Progress Indicator」: 不定形（Circular）と確定型（Linear＋割合のテキスト）の2種を持つことにする。確定型は音声結合のログ行だけで使う。割合はテキストでも併記する（色やバーの長さだけに頼らない）
- 5.1 `PipelineState` の `processing` に `progress: Record<string, number>`（ステップ名 → 0〜1）を追加する
- 7章:
  - `run_ffmpeg_concat` に進捗通知用の Channel 引数を追加する
  - `AudioConcatInput` に `onProgress?: (ratio: number) => void` を追加する
  - `PipelineCallbacks` に `onStepProgress: (stepName: string, ratio: number) => void` を追加する
  - `pipelineController` は `onStepProgress` を `progress` に反映する
- 8章 `ProcessingView`: 音声結合の行だけ、処理中にバーと割合を表示する。「1/4」のような段階表示をしない方針は維持する

### 4.2 imple（設計）
- Rust ドメイン層は Tauri に依存させないまま、進捗をコールバック（`&mut dyn FnMut(f64)`）で受け取る
  - `process.rs`: stdout を1行ずつコールバックに渡す実行関数を追加する（タイムアウト・子プロセスの終了処理は既存の `run_with_timeout` と共通にする）
  - `audio/`: `-progress` の出力行を解釈する純粋関数（`out_time_us` の取り出し）と、2段階の割合を計算する純粋関数を追加し、単体テストの対象にする
  - `audio/concat.rs`: `concat_audio` に進捗コールバックを追加する
- コマンド層 `commands/audio_process.rs`: `tauri::ipc::Channel<f64>` を受け取り、コールバックから `send` する
- TypeScript:
  - `tauriCommands.ts`: `runFfmpegConcat` に進捗コールバックを受け取らせ、内部で `Channel` を作って渡す
  - `audioConcatStep.ts`: `onProgress` を `runFfmpegConcat` に渡す
  - `orchestrator.ts`: 音声結合の実行時に `onProgress` として `cb.onStepProgress(audio.name, ratio)` を渡す
  - `pipelineController.ts`: `progress` を更新する。ステップ完了時はそのステップの値を消す
- UI:
  - `ui/ProgressIndicator.svelte`: `value?: number`（0〜1）を追加する。`value` があれば Linear（確定型）でバーと「42%」を表示し、`role="progressbar"`・`aria-valuenow` 等を付ける。なければ今の Circular（不定形）
  - `ui/ListItem.svelte`: `progress?: number` を追加し、処理中の行で ProgressIndicator に渡す
  - `StepLog.svelte`: `progress` を受け取り、実行中の行へ渡す
  - `prefers-reduced-motion: reduce` のときは、バーの伸びのアニメーションをなくす（値の更新は表示する）

### 4.3 tasks・walk（Phase構成）
- 現行の Phase 7（リリース準備、T7-1〜T7-4・W7-1〜W7-3）はすべて未着手。リリース準備は最終Phaseに置くルールのため、新しい Phase 7 の後ろへ移す
  - グローバルルールの「並べ替え」に従い、移動する項目には未使用の新しいIDを振る: T7-1〜T7-4 → **T8-1〜T8-4**、W7-1〜W7-3 → **W8-1〜W8-3**。旧IDの位置には移動先を1行で残す
- **新 Phase 7: 音声結合の進捗表示**（IDは T7-5〜、W7-4〜。T7-1〜T7-4・W7-1〜W7-3 は移動先の記録として欠番扱いにする）
  - T7-5 【エージェント】SPEC v3 の作成（本changeの承認後の手順の一部として実施）
  - T7-6 【エージェント】`process.rs` に stdout を行ごとに渡す実行関数を追加し、単体テストを書く
  - T7-7 【エージェント】`-progress` 出力の解釈と2段階の割合計算の純粋関数を追加し、単体テストを書く
  - T7-8 【エージェント】`concat_audio` と `run_ffmpeg_concat` に進捗通知を組み込む。実サンプルで copy・再エンコードそれぞれの所要時間を測り、2段階の重みを決める
  - T7-9 【エージェント】TypeScript側（tauriCommands・AudioConcatStep・Orchestrator・Controller・PipelineState）を変更し、Vitestの単体テストを追加・更新する
  - T7-10 【エージェント】ProgressIndicator（確定型）・ListItem・StepLog を変更し、`/dev/ui`・`/dev/screens` に確定型の見本を追加する
- **検証**
  - W7-4 【エージェント】`cargo test`・`bun run check`・`bun run test` が通る
  - W7-5 【エージェント】ffmpeg の結合テスト（`#[ignore]`）で、通知された割合が 0〜1 の範囲で単調に増え、最後が 1 になることを確認する（copy・再エンコードの両方）
  - W7-6 【ユーザー】実サンプル（pptx・ppsx）をドロップし、音声結合の行だけにバーと割合が表示されて進み、ほかの行は今までの表示のままであることを確認する
  - W7-7 【ユーザー】音声形式不一致版（再エンコード）でも割合が戻らずに進むことを確認する
  - W7-8 【ユーザー】「アニメーション効果」をオフにしたとき、バーが動きなしで更新されることを確認する
  - W7-9 【エージェント】追加・変更したコンポーネントに色・影・角丸・余白の直書きがないことを確認する
- Phase 6 の未完了の walk（W6-5〜W6-13）は、本プロジェクトでは Phase の同期を適用しない取り決め（`261002133633_change_sample_formats_and_release.md`）のため、Phase 7 と前後してもよい。ただし W6-12（リリースビルドでの受け入れ確認）は、Phase 7 の反映後のビルドで行うことを推奨する

## 5. 影響しないもの

- 成果物（m4a・pdf・json）の内容と出力先、タイムスタンプの計算
- pptx解析・PDF変換・JSON書き出しの表示（不定形のまま）
- `tokens.css`・brand.md・アイコン

## 6. 承認後の手順

1. 現行の imple・tasks・walk（`261002133708〜133710`）と SPEC v2 を `roadmap/archived/` へ移す
2. SPEC v3 と、新タイムスタンプの imple・tasks・walk を作成する（完了済みの項目はIDとチェック状態を引き継ぐ）
3. 全項目の依存先が前方にあることを確認し、T7-6 から着手する
