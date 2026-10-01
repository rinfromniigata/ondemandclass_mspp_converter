# 仕様変更サマリー（change）: デザインシステムの導入

- 状態: **承認待ち**
- 変更元: リポジトリ直下の `ondemandclass_mspp_converter_SPEC.md`（3章「デザインシステム」）、
  `ondemandclass_mspp_converter_tokens.css`、`ondemandclass_mspp_converter_icon_master.svg`
- 取り込み方針: 直下のスペックシートからは**3章（デザインシステム）のみ**を取り込む。
  それ以外の章は初版のままの記述が残っているため取り込まず、現行v1（合意済みの仕様変更を反映済み）を正とする

---

## 1. 変更内容

### 1.1 スペックシートに追加するもの（v2で新3章として挿入。以降の章番号を1つずつ繰り下げる）
- デザインアセットの同梱：トークン（カラー・タイポグラフィ・余白・角丸・影・モーション・状態レイヤー）、アイコン原本（1024×1024）
- Material Design 3 を土台とし、M3のロールを `tokens.css` の既存トークン名に対応付けて使う
- アクセシビリティ原則
  - 本文コントラスト比4.5:1以上
  - `:focus-visible` でフォーカスリングを表示（`--color-primary-strong`、2px、オフセット2px）
  - クリック可能領域は最小40×40px
  - 状態は色だけで示さず、アイコンとテキストラベルを併記する
  - `prefers-reduced-motion: reduce` ではモーションの duration を0にする
- ウィンドウの最小サイズ480×360px、常に1カラム
- elevation は3段階（0〜3）、shape は4段階に簡略化。状態レイヤーはM3標準値
- 基本部品7種：Button（Filled/Text）、Card、List item、Progress Indicator、Dialog、Banner、Status Chip
- 確定事項・委任事項の区分（直下のスペックシートの記載どおり）

### 1.2 デザインシステムを取り込むために必要になる、既存仕様の変更
1. **上書き確認をアプリ内のダイアログにする**
   - 部品「Dialog」は、M3のBasic Dialog（elevation 3・スクリム付き、ボタンはFilledの「上書きする」とTextの「キャンセル」）として確定している。OS標準のダイアログ（`tauri-plugin-dialog` の `ask`）ではこの見た目にできない
   - そのため、Svelteで作るアプリ内ダイアログに置き換え、`tauri-plugin-dialog` は使わない
   - `pipelineController` の `confirm` に渡す実装は、Promiseを返すダイアログ用ストア（`confirmDialog.ts`）にする
2. **実行中のステップを状態として持つ**
   - 部品「Progress Indicator」は、`ExtractPptxStep` の実行中は画面全体に1つ、音声結合とPDF変換は各ログ行に個別に表示すると決まっている。そのためには、どのステップが実行中かを知る必要がある
   - `PipelineState` の `processing` に `running: string[]` を追加する
   - `PipelineOrchestrator.run` に、各ステップの開始時に呼ぶ `onStart(stepName)` を追加する
3. **警告とエラーの表示方法を部品に合わせる**
   - ステップ単位の警告（リンク切れ、動画ナレーション、再エンコード）と、idle画面の設定エラー・受付拒否理由は「Banner」で表示する
   - ログ行（List item）の状態は成功・失敗・処理中の3つにする。警告があるステップは成功アイコンに「完了（警告あり）」のラベルを付け、詳細はBannerに出す
   - v1の「警告付き成功＝黄色」は廃止する（`--color-warning` は部品の役割に割り当てられていないため）
4. **ResultView に「ログ詳細」を追加する**
   - Text Buttonの用途に「ログ詳細」が挙げられているため、ResultViewに処理ログ（`results`）の表示を切り替えるボタンを付ける
5. **Status Chip の文言**
   - `done` は「完了」、`error` で成果物が1つ以上ある場合は「一部エラー」
   - 成果物が1つもない場合（pptx解析の失敗など）は「失敗」とする（エージェント判断。2章 Q3）
6. **アイコンを生成する**
   - アイコン原本から `bun tauri icon` で `src-tauri/icons/` 一式を作る
7. **アセットの置き場所**
   - `tokens.css` は値を変えずに `src/lib/styles/tokens.css` へ移し、ルートレイアウトで読み込む
   - トークンの値は確定事項なので、reduced-motion などの上書きは別ファイル `src/lib/styles/base.css` に書く
   - アイコン原本は `assets/design/ondemandclass_mspp_converter_icon_master.svg` へ移す。`assets/design/ai/` は既に `.gitignore` で除外されているため、それとは別にGit管理する
   - 直下の `ondemandclass_mspp_converter_SPEC.md` は、取り込みが終わったら `roadmap/archived/` へ移す
8. **テーマ**
   - ダークモードはOSの設定に従う（`tokens.css` の `prefers-color-scheme` による）
   - `data-theme` を手動で切り替えるUIはv2では作らない

---

## 2. 確認事項（承認時にご回答ください）

- **Q1. ライトモードのエラー色のコントラスト不足**（確定事項どうしが食い違っている）
  - Bannerの「文字は `--color-error`」と、本文の「コントラスト比4.5:1以上」を同時には満たせない
  - 実測値（WCAGの計算式）
    - `--color-error` #e48b97 を白背景に置いた場合：2.49:1
    - Banner背景（エラー色12%混合）に置いた場合：2.25:1
    - ダークモードは4.74:1で、基準を満たしている
  - **推奨案**：Bannerの文字は `--color-text` にし、`--color-error` はアイコンと左端の帯（またはボーダー）だけに使う。トークンの値は変えない
  - 別案：エラー文字用のトークン（例 `--color-error-text`）を `tokens.css` に追加する（確定事項の変更になる）
- **Q2. 色だけで判別させない部品のコントラスト**（参考情報。数値の基準外のため、変更は提案しない）
  - ライトモードで白背景に置いた場合の実測値
    - `--color-success`：1.52:1
    - `--color-primary-strong`（フォーカスリング）：2.72:1
    - `--color-primary`（Progress Indicator）：1.84:1
  - いずれも WCAG 1.4.11（UI部品は3:1）を下回る。ただし、ログ行はアイコンとテキストラベルを併記するため、状態の判別はラベルで担保できる
  - フォーカスリングの見やすさは、walkの目視確認で判断したい
- **Q3.** 1.2-5 で、成果物がないときのStatus Chipを「失敗」にしてよいか
- **Q4.** 直下のスペックシートが参照している `ondemandclass_mspp_converter_brand.md` が、リポジトリにない
  - 実装には直接影響しないため、後回しにしてよいか
  - 追加予定があれば、`assets/design/` に置いていただければ参照する

---

## 3. 影響するドキュメントと項目

### スペックシート（v1 → v2）
- 新3章「デザインシステム」を挿入し、旧3〜14章を4〜15章に繰り下げる
- 1章の技術スタックから `tauri-plugin-dialog` を削除し、「デザイントークン」と「アイコン生成」を追加する
- 4章（旧3章）のディレクトリ構成に以下を追加する
  - `src/lib/styles/`（tokens.css・base.css）
  - `src/lib/components/ui/`（基本部品7種）
  - `src/lib/confirmDialog.ts`
  - `src/routes/+layout.svelte`
  - `src-tauri/icons/`
  - `assets/design/`
- 5章（旧4章）：`PipelineState.processing` に `running` を追加する
- 7章（旧6章）
  - Orchestrator に `onStart` を追加する
  - Controller の確認処理をアプリ内ダイアログにする
- 8章（旧7章）：各画面の表示を、部品（Card・List item・Progress・Banner・Status Chip・Dialog・ログ詳細）に沿って書き直す
- 9章（旧8章）-6：確認ダイアログはアプリ内のDialogと明記する
- 13章（旧12章）の受け入れ基準に追加する項目
  - キーボード操作でフォーカスリングが見えること
  - 状態がアイコンとラベルで示されること
  - reduced-motion
  - 最小ウィンドウサイズ
  - ダークモード追従

### imple（新タイムスタンプで作り直す）
- 3.9 lib.rs、3.10 capabilities：dialog プラグインと権限を削除する
- 3.11 tauri.conf.json：`minWidth: 480`、`minHeight: 360` を追加する
- 4.5 orchestrator：`onStart` を追加する
- 4.6 controller：`confirm` をアプリ内ダイアログにする
- 4.8 コンポーネント：画面を部品で組む方針に変える
- 新節「デザインシステムの実装」
  - トークンの読み込み
  - 状態レイヤー（`color-mix()`）
  - `:focus-visible`
  - reduced-motion
  - 部品7種のProps
  - アイコン生成

### tasks（完了済みの項目はないため、チェック状態の引き継ぎはなし）
- 内容を変える項目
  - T1-4：`tauri-plugin-dialog` を外す
  - T1-5：最小サイズを追加し、dialog の権限を外す
  - T5-1：`running` を追加する
  - T5-4：`onStart` を追加する
  - T5-5：確認をアプリ内ダイアログにする
  - T5-6：`onStart` / `running` / 確認ダイアログのテストを追加する
  - T5-7：画面を部品で組む
- 新しい項目（仮番号）
  - T1-9 【エージェント】デザインアセットを配置する（tokens.css・アイコン原本）
  - T1-10 【エージェント】`bun tauri icon` でアプリアイコンを生成する
  - T5-8 【エージェント】`base.css`（リセット・フォーカスリング・状態レイヤー・reduced-motion）と `+layout.svelte` を作る
  - T5-9 【エージェント】基本部品7種を `src/lib/components/ui/` に実装する
  - T5-10 【エージェント】`confirmDialog.ts` を実装する
- T5-7 は T5-8〜T5-10 に依存させる

### walk（新タイムスタンプで作り直す）
- 内容を変える項目
  - W6-9：アプリ内ダイアログ（「上書きする」／「キャンセル」）で確認する
  - W6-5 / W6-6：Bannerで表示されることを確認する
- 新しい項目（仮番号）
  - W1-4 【ユーザー】タスクバーとウィンドウにアプリアイコンが表示される
  - W5-4 【ユーザー】Tabキーだけで操作でき、すべての操作要素にフォーカスリングが表示される
  - W5-5 【ユーザー】ウィンドウを480×360pxより小さくできない
  - W5-6 【ユーザー】OSをダークモードにすると配色が切り替わる
  - W5-7 【ユーザー】OSの「アニメーション効果」をオフにすると、遷移アニメーションが止まる
  - W5-8 【エージェント】主要な文字色と背景色の組み合わせのコントラスト比を計算し、4.5:1以上であることを記録する

---

## 4. 承認後の手順
1. 現行の imple・tasks・walk・`SPEC_v1.md` を `roadmap/archived/` へ移す
2. `SPEC_v2.md` を作成し、imple・tasks・walk を新タイムスタンプで作成する
3. アセットを移す作業（1.2-7）はT1-9として扱い、ドキュメント更新の時点ではファイルを動かさない
   （直下の `ondemandclass_mspp_converter_SPEC.md` だけは、ドキュメント更新時に `archived/` へ移す）
