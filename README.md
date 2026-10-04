# オンデマンドスライドコンバーター

ナレーション付きの PowerPoint ファイル（`.pptx` / `.ppsx`）を1つドラッグ＆ドロップするだけで、文字起こしや資料化に使える3つのファイルを生成する Windows 向けデスクトップアプリです。

オンデマンド授業で配布される、スライド切り替えとナレーション音声が埋め込まれた PowerPoint ファイルをそのまま扱えます。

## 生成されるファイル

入力ファイルと同じフォルダに、次の3つを書き出します（`<basename>` は入力ファイル名から拡張子を除いたもの）。

- `<basename>_audio.m4a` … 各スライドのナレーションをスライド順に結合した音声
- `<basename>_slides.pdf` … スライドをそのまま書き出したテキスト層付き PDF（OCR 不要）。PowerPoint の音声の再生アイコンは除去して書き出します
- `<basename>_timestamps.json` … 結合した音声の中で、各スライドが始まる・終わる時刻の対応表

```json
[
  { "slide": 1, "startSec": 0, "endSec": 42.5 },
  { "slide": 2, "startSec": 42.5, "endSec": 97.1 }
]
```

同じ名前のファイルがすでにある場合は、上書きしてよいか確認します。

## このアプリが行わないこと

- 文字起こし・要約などの後続の処理
- ネットワーク通信（外部へのアップロードや連携は行いません。処理はすべてローカルで完結します）
- フォルダの監視による自動実行（変換は常にドラッグ＆ドロップで始めます）
- `.pptx` / `.ppsx` 以外の形式（`.ppt`・`.pptm`・Keynote 等）、複数ファイルの一括変換
- 動画として埋め込まれたナレーションの音声の取り出し（警告を出し、そのスライドは無音として扱います）

## 動作環境

- Windows（x64）
- macOS 版もリリースに含まれますが、ビルドのみで、変換の動作は保証の対象外です

### 必要なソフト

次のソフトを別途インストールしてください。アプリはこれらをローカルで呼び出します。

- [FFmpeg](https://ffmpeg.org/)（`ffmpeg` と `ffprobe`）… 音声の解析と結合
- [LibreOffice](https://www.libreoffice.org/)（`soffice`）… PDF への変換

## 使い方

### 1. インストール

[Releases](https://github.com/rinfromniigata/ondemandclass_mspp_converter/releases) から Windows 用のインストーラー（`.exe` または `.msi`）をダウンロードして実行します。

### 2. 設定ファイルを置く

アプリの実行ファイル（`ondemandclass_mspp_converter.exe`）と同じフォルダに `app.settings.json` を置きます。このリポジトリの [`app.settings.example.json`](app.settings.example.json) をコピーし、自分の環境のパスに書き換えてください。

```json
{
  "ffmpegPath": "C:\\ffmpeg\\bin\\ffmpeg.exe",
  "ffprobePath": "C:\\ffmpeg\\bin\\ffprobe.exe",
  "sofficePath": "C:\\Program Files\\LibreOffice\\program\\soffice.exe",
  "silentSlideHandling": "insert_silence",
  "silentSlideDefaultSec": 3,
  "audioReencodeOnMismatch": true
}
```

- `ffmpegPath` / `ffprobePath` / `sofficePath`（必須）… 各実行ファイルのパス。JSON の中では `\` を `\\` と書きます
- `silentSlideHandling` … ナレーションのないスライドの扱い
  - `"insert_silence"`（既定）… 無音を挿入して、時刻の対応を保つ
  - `"skip"` … 結合音声から省く（JSON では開始と終了が同じ時刻になります）
- `silentSlideDefaultSec` … `insert_silence` のとき、自動切り替え時間が設定されていないスライドに挿入する無音の秒数（既定 `3`）
- `audioReencodeOnMismatch` … スライド間で音声の形式が異なるとき、再エンコードして結合するか（既定 `true`。`false` の場合は変換を失敗として止めます）

設定ファイルが見つからない、またはパスが誤っている場合は、起動画面に内容が表示されます。修正してから「設定を再読み込み」を押してください。

### 3. 変換する

1. アプリを起動し、`.pptx` または `.ppsx` ファイルをウィンドウにドラッグ＆ドロップします
2. 処理の経過が表示されます（音声の結合は進み具合を % で表示します）
3. 完了すると、生成したファイルの一覧と警告が表示されます。「出力フォルダを開く」で保存先を開けます

## 開発

### 必要なツール

- [Bun](https://bun.sh/)
- [Rust](https://www.rust-lang.org/)（stable）
- Windows では Microsoft C++ Build Tools と WebView2（[Tauri の前提条件](https://v2.tauri.app/start/prerequisites/)を参照）
- 動作確認用に FFmpeg と LibreOffice

### セットアップと起動

```bash
bun install
cp app.settings.example.json app.settings.json   # 自分の環境のパスに書き換える
bun run tauri dev
```

開発時は、リポジトリ直下の `app.settings.json` を読み込みます。

### テスト

```bash
bun run test                                     # フロントエンド（Vitest）
cargo test --manifest-path src-tauri/Cargo.toml  # Rust
```

FFmpeg・LibreOffice を実際に呼び出すテストは `#[ignore]` にしてあり、上のコマンドでは実行されません。

- 音声結合の結合テスト: 環境変数 `FFMPEG_PATH`・`FFPROBE_PATH` に各実行ファイルのパスを設定し、`cargo test --manifest-path src-tauri/Cargo.toml --test concat_integration -- --ignored` を実行します
- `derived_samples`・`pdf_derived`: 手元の実ファイル（`samples/`、リポジトリには含めません）から `scripts/make_samples.ts` で作るサンプルを使います。手順は各テストファイル冒頭のコメントを参照してください

### ビルド

```bash
bun run tauri build
```

`src-tauri/target/release/` に実行ファイルが、`src-tauri/target/release/bundle/` にインストーラーが生成されます。実行するときは、実行ファイルと同じフォルダに `app.settings.json` を置いてください。

### 技術スタック

Tauri v2 / Rust / SvelteKit（Svelte 5）/ TypeScript / Bun / Vitest

## 変更履歴

[CHANGELOG.md](CHANGELOG.md) を参照してください。

## ライセンス

[Apache License 2.0](LICENSE)
