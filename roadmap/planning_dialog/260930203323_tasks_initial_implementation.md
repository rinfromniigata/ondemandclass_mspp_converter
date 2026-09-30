# 実装タスク（tasks）: 初期実装

- 対象スペック: `roadmap/specs/ondemandclass_mspp_converter_SPEC_v1.md`
- 実装設計: `roadmap/planning_dialog/260930203148_imple_initial_architecture.md`
- 検証手順: `roadmap/planning_dialog/260930203357_walk_initial_implementation.md`

各Phaseの完了後、対応するwalkのPhaseで検証してから次のPhaseへ進む。

---

## Phase 1: 環境構築
- [ ] T1-1 【エージェント】開発環境を確認する（bun・rustc/cargo・ffmpeg・ffprobe・soffice のバージョンと実行ファイルパス、WebView2の有無）
  - 結果を実施メモに残す
- [ ] T1-2 【ユーザー】T1-1で不足が見つかったツールをインストールする
  - 依存: T1-1
  - 不足がなければ実施不要としてチェックする
- [ ] T1-3 【エージェント】スクラッチディレクトリでTauri（svelte-ts）テンプレートを生成し、リポジトリ直下へ取り込む
  - 依存: T1-2
  - 手順は imple 6章。`.gitignore` は既存内容とマージする
  - adapter-static（`fallback: "index.html"`）と `+layout.ts` の `ssr = false` を確認・修正する
  - テンプレートのサンプル（greet・ロゴ・サンプルCSS）を削除する
- [ ] T1-4 【エージェント】依存関係を追加する
  - 依存: T1-3
  - Rust: `zip` `quick-xml` `tempfile` `serde` `serde_json` `tauri-plugin-dialog` `tauri-plugin-opener`
  - JS: `@tauri-apps/plugin-dialog` `@tauri-apps/plugin-opener`、dev: `vitest`
  - `package.json` に `"test": "vitest run"` を追加し、`vite.config.ts` にテスト設定を追加する
- [ ] T1-5 【エージェント】`tauri.conf.json` と `capabilities/default.json` を設定する
  - 依存: T1-4
  - 識別子・productName・ウィンドウタイトル・サイズは imple 3.11、権限は imple 3.10
- [ ] T1-6 【エージェント】`app.settings.example.json` を作成し、`.gitignore` に `app.settings.json` と `samples/` を追加する
  - 依存: T1-3
- [ ] T1-7 【エージェント】T1-1で確認したパスを使って `app.settings.json` を作成する
  - 依存: T1-1, T1-6
- [ ] T1-8 【ユーザー】手動検証用に、実際に配布されたpptx（あればppsxも）を `samples/` に置く
  - 手動検証（walk Phase 6）までに用意できればよい。Phase 2以降の着手を妨げない

## Phase 2: Rust基盤
- [ ] T2-1 【エージェント】`state.rs`・`error.rs`・`process.rs`（`run_with_timeout`）を実装する
  - 依存: T1-4
  - CREATE_NO_WINDOW、stdout/stderr の別スレッド読み取り、タイムアウト時の kill を含む
  - `process.rs` の単体テスト（正常終了・非ゼロ終了・タイムアウト・起動失敗）を書く
- [ ] T2-2 【エージェント】`settings/`（探索・解析・検証）とコマンド `load_and_validate_settings` を実装する
  - 依存: T2-1
  - `parse_settings` / `validate` の単体テスト（省略項目のデフォルト、不正値、パス不在）を書く
- [ ] T2-3 【エージェント】`lib.rs` にプラグイン・State・コマンドを登録し、`main.rs` を整える
  - 依存: T2-2
  - この時点では未実装のコマンドは登録しない（Phaseごとに追加する）

## Phase 3: pptx解析
- [ ] T3-1 【エージェント】`pptx/package.rs`・`pptx/rels.rs` を実装する
  - 依存: T2-1
  - パス正規化（`../media/x.m4a`）と `TargetMode="External"` の判定に単体テストを書く
- [ ] T3-2 【エージェント】`pptx/presentation.rs`（sldIdLst による順序解決）を実装する
  - 依存: T3-1
- [ ] T3-3 【エージェント】`pptx/slide.rs`（図形・p:timing・advTm・動画検出）を実装する
  - 依存: T3-1
  - `parse_slide` は XML文字列を入力にする純粋関数とし、単体テストを書く
- [ ] T3-4 【エージェント】`pptx/mod.rs`（`extract_slide_audio_map`）とコマンド `extract_pptx` を実装し、登録する
  - 依存: T3-2, T3-3
- [ ] T3-5 【エージェント】テスト用 `PptxBuilder` を作成し、imple 5章の主なケースの結合テストを書く
  - 依存: T3-4
  - 受け入れ基準「sldIdLst順」「p:timing順」をこのテストで満たす

## Phase 4: 音声結合・PDF変換・JSON書き出し
- [ ] T4-1 【エージェント】`audio/probe.rs`（ffprobe実行と `parse_probe_json`）を実装する
  - 依存: T2-1
- [ ] T4-2 【エージェント】`audio/plan.rs`（結合方式判定）と `audio/timeline.rs`（タイムスタンプ計算）を実装し、単体テストを書く
  - 依存: T4-1
  - timeline は skip 時の長さ0区間、複数区間のスライド、先頭が無音のケースを含める
- [ ] T4-3 【エージェント】`audio/concat.rs` とコマンド `run_ffmpeg_concat` を実装し、登録する
  - 依存: T3-1, T4-2
- [ ] T4-4 【エージェント】ffmpegを使う結合テスト（`#[ignore]`）を書き、実行する
  - 依存: T4-3, T1-7
  - サンプルレート不一致・チャンネル数不一致・MP3混在で再エンコードに切り替わること、無音挿入、一時ディレクトリが残らないことを確認する
- [ ] T4-5 【エージェント】`pdf/mod.rs` とコマンド `run_soffice_convert` を実装し、登録する
  - 依存: T2-1
  - アプリ専用プロファイル、一時フォルダ出力→移動、120秒タイムアウト
- [ ] T4-6 【エージェント】コマンド `check_outputs_exist`・`write_timestamps_json` を実装し、登録する
  - 依存: T2-3
  - JSONの丸め（小数3桁）と書き出し形式に単体テストを書く

## Phase 5: フロントエンド
- [ ] T5-1 【エージェント】`steps/types.ts`・`tauriCommands.ts` を実装する
  - 依存: T1-4
- [ ] T5-2 【エージェント】`outputPaths.ts`・`settings.ts`・`pipelineStore.ts` を実装する
  - 依存: T5-1
- [ ] T5-3 【エージェント】4つのStep（`buildSegments` を含む）を実装する
  - 依存: T5-1
- [ ] T5-4 【エージェント】`orchestrator.ts` を実装する
  - 依存: T5-3
- [ ] T5-5 【エージェント】`pipelineController.ts` を実装する
  - 依存: T5-2, T5-4
- [ ] T5-6 【エージェント】Vitestの単体テストを書く（outputPaths・buildSegments・orchestrator・pipelineController）
  - 依存: T5-5
- [ ] T5-7 【エージェント】`+layout.ts`・`+page.svelte` と4つのコンポーネントを実装する
  - 依存: T5-5

## Phase 6: 統合・仕上げ
- [ ] T6-1 【エージェント】静的チェックと全テストを実行し、指摘をすべて解消する
  - 依存: Phase 2〜5
  - `cargo fmt --check` / `cargo clippy` / `cargo test` / `cargo test -- --ignored` / `bun run check` / `bun run test`
- [ ] T6-2 【エージェント】手動検証用の派生サンプルを作るスクリプト `scripts/make_samples.ts` を作成し、実行する
  - 依存: T1-8, T4-3
  - `samples/` の実pptxを元に、ppsx版・リンク切れ版（relsを `TargetMode="External"` に書き換え）・音声形式不一致版（1パートを別サンプルレートに再エンコードして差し替え）・音声なし版を `samples/derived/` に生成する
  - 必要に応じて devDependency に zip操作ライブラリを追加する
- [ ] T6-3 【エージェント】ppsxがsofficeでPDF化できることを確認する（スペック14章）
  - 依存: T4-5, T6-2
  - できない場合は入力フィルタ指定等で対処し、fixとして記録する
- [ ] T6-4 【エージェント】`bun run tauri build` でリリースビルドを作成し、実行ファイルと同じフォルダの `app.settings.json` が読まれることを確認する
  - 依存: T6-1
