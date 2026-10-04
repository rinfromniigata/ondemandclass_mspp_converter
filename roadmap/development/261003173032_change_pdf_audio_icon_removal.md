# 仕様変更サマリー（change）: PDFから音声の再生アイコンを除去する

- 状態: **承認済み**（261004、チャットでの承認。動画の図形は残す・設定項目は設けない、の2点も了承）
- 反映先: `roadmap/specs/ondemandclass_mspp_converter_SPEC_v4.md`（新版）、`roadmap/planning_dialog/261004085952_imple_pdf_audio_icon_removal.md`・`261004085952_tasks_pdf_audio_icon_removal.md`・`261004085953_walk_pdf_audio_icon_removal.md`
- 反映時の補足（imple・tasks・walk 作成時に詰めた点。4.4の項目案から変えた箇所）
  - スペックv4の作成を完了済みの T9-1 とし、実装項目を T9-2〜T9-6 とした（v3 の T7-5 と同じ扱い）
  - soffice を使う結合テスト `tests/pdf_derived.rs`（`#[ignore]`、`SOFFICE_PATH`）を T9-5 として追加した。W9-2 はこのテストで確認する
  - 入力が変更されないことの【ユーザー】確認を W9-4 として分けた。walk は W9-1〜W9-5
  - soffice はエージェントのサンドボックス内では異常終了する（未加工の入力でも同様）ため、soffice を使う確認はサンドボックス外で実行すると tasks・imple に明記した
- デザインアセット（`assets/design/` の tokens・brand・アイコン原本）: **変更なし**

---

## 1. 背景と要望

- 書き出した `<basename>_slides.pdf` に、PowerPointで埋め込み音声に表示される再生アイコン（スピーカー画像）がそのまま描かれ、その下のテキストや画像が隠れていた
- ユーザーの要望（261003）: PDFを直接読むときに邪魔なので、再生アイコンを除去する機能を追加したい

## 2. 原因の調査結果

- サンプル（`samples/` の前編pptx）では、音声は各スライドの `p:spTree` 内の `p:pic` 要素（`p:nvPicPr/p:nvPr/a:audioFile` を持つ）として置かれ、見た目は約5.7cm角の円形画像（`a:blip` が `ppt/media/image1.png`）になっている
- LibreOffice はこの `p:pic` を**アイコン画像つきのメディア図形**として読み込み、PDFにも次のものを出力していた（スペックv3の「LibreOfficeは埋め込み音声を無視する」という記述は誤り）
  - アイコン画像（下の要素を覆う）
  - 全33ページに再生用の注釈（`/Subtype/Screen`）と、音声データ本体の埋め込み（`/EmbeddedFile`）
- このため、元の出力PDFは約85MBあり、そのほとんどが音声データだった

## 3. 方式の検証（261003、サンプル前編pptx、LibreOffice 既存インストール版）

pptxの一時コピーを加工してから soffice に渡す方式を2通り試した。

- A. 音声の `p:pic` 要素を削除する
- B. 音声の `p:pic` の `p:cNvPr` に `hidden="1"` を付ける

結果（33ページ）:

- 元のまま: PDF 84.8MB／Screen注釈 33／埋め込み音声 33／画像XObject 85
- A（削除）: PDF 2.3MB／Screen注釈 0／埋め込み音声 0／画像XObject 81
- B（hidden）: PDF 2.3MB／Screen注釈 0／埋め込み音声 0／画像XObject 81

どちらも同じ結果になった。**A（削除）を採用する**。

- 理由: B は LibreOffice が `hidden` を「PDFに出さない」と解釈することに依存するが、A は図形そのものがないため、LibreOffice の版による解釈の違いを受けない
- A では `p:timing` 内の `p:spTgt spid` が存在しない図形を指すが、PDFはアニメーションを使わないため影響しない（検証でも変換は正常終了した）
- ページの見た目（アイコンが消え、下のテキスト・画像が見えること）は、エージェント環境にPDFの画像化ツールがないため未確認。walk の【ユーザー】項目で確認する

## 4. 変更内容

### 4.1 振る舞い

- PDF変換で、**音声の図形（`a:audioFile` を持つ `p:pic`）を常に除去**してから変換する。設定項目は設けない（成果物の目的は「直接読む一次資料」であり、アイコンを残す用途がないため）
- 入力ファイル自体は変更しない。加工は一時フォルダのコピーに対して行う
- **動画の図形（`a:videoFile`）は除去しない**（動画ナレーションは非スコープで、動画の表紙画像はスライド内容の一部である可能性があるため）
- 音声図形のないスライドのXMLは書き換えない
- 加工に失敗した場合（zip・XMLが読めない等）は、PDF変換ステップを失敗として理由を表示する（アイコンつきのPDFを黙って出さない）
- 副次効果: PDFに音声データが入らなくなり、ファイルサイズが大幅に小さくなる（サンプルで約85MB→約2.3MB）

### 4.2 設計（imple に反映）

- Rust の `pdf/` に加工処理を追加する。**コマンド・TypeScript・画面は変更しない**（`run_soffice_convert` の引数・戻り値はそのまま）
  - `pdf/strip_audio.rs`（新規）
    - `strip_audio_shapes(xml: &str) -> Result<Option<String>, AppError>`（純粋関数）: スライドXMLから音声の `p:pic` を取り除いた文字列を返す。音声図形がなければ `None`
      - quick-xml で読み、`p:pic` の開始〜終了のバイト範囲を記録し、その中の `nvPicPr/nvPr` 直下に `audioFile` があるものだけを切り取る（判定条件は `pptx/slide.rs` と同じ親要素チェック）
      - `mc:AlternateContent` の中にある場合も、各分岐の `p:pic` をそれぞれ判定して除去する
    - `write_pdf_source(input: &Path, dest: &Path) -> Result<(), AppError>`: 入力のzipを `dest` へ書き直す。`ppt/slides/slide*.xml` は `strip_audio_shapes` の結果で置き換え、それ以外のエントリは `raw_copy_file` で再圧縮せずにコピーする
  - `convert_to_pdf` の変更: 一時フォルダ内に入力と同じファイル名で加工済みコピーを作り、それを soffice に渡す（出力名 `<basename>.pdf` の規則は変わらない）
- 影響の局所化: 「PDFから除く図形の判定」は `pdf/strip_audio.rs` のみ

### 4.3 スペックシート（v4）

- 改訂履歴に v4 を追加
- 6章 `run_soffice_convert`: 責務の先頭に「音声図形を除去した一時コピーを作る」手順を追加。備考の誤り（埋め込み音声を無視する）を、除去の理由に書き換える
- 9章（エッジケース）: 「音声の再生アイコンはPDFに出さない。動画の図形は残す。入力ファイルは変更しない」を追加
- 13章（受け入れ基準・機能）: 「PDFに音声の再生アイコンが描かれず、下のテキスト・画像が見える」「PDFに音声データが埋め込まれない」を追加
- 0章・3章・デザインシステムの確定事項: 変更なし

### 4.4 tasks・walk

- 現在 Phase 7（音声結合の進捗表示）の W7-4〜W7-10 が未完了のため、**この変更の実装は W7 がすべて通過した後**に行う
- 新しい Phase 8 は既存の「リリース準備」の番号と重なるため、次のように並べ替える（ルールに従い、未着手のリリース準備の項目に新しいIDを振り、旧IDの位置には移動先を1行で残す）
  - **Phase 9: PDFの再生アイコン除去**（新規）
  - **Phase 10: リリース準備**（旧 Phase 8 の T8-1〜T8-4・W8-1〜W8-3 を T10-1〜T10-4・W10-1〜W10-3 へ移動）
- Phase 9 の項目案
  - T9-1 【エージェント】`pdf/strip_audio.rs` に `strip_audio_shapes` を実装し、単体テスト（音声1つ・複数・なし・動画のみ・AlternateContent内・`a:audioFile` 以外の同名要素）を書く
  - T9-2 【エージェント】`write_pdf_source` を実装し、テスト（音声図形の除去・ほかのエントリが同一バイトで残る・入力ファイルが変更されない）を書く
    - 依存: T9-1
  - T9-3 【エージェント】`convert_to_pdf` を加工済みコピー経由に変更する
    - 依存: T9-2
  - T9-4 【エージェント】リリースビルドを作り直す
    - 依存: T9-3
  - W9-1 【エージェント】`cargo test`・`bun run check`・`bun run test` が通る
  - W9-2 【エージェント】派生サンプル（pptx・ppsx の original）を変換し、PDFに Screen 注釈・埋め込み音声がないこと、ページ数が元と同じこと、PDFのサイズが小さくなったことを確認する
  - W9-3 【ユーザー】`bun run tauri dev` で pptx・ppsx をそれぞれ変換し、PDFに再生アイコンが描かれず、下のテキスト・画像が見えることを確認する
  - W9-4 【ユーザー】リリースビルドで同じ確認をする
- T10-1（旧 T8-1）は T9-4 に依存させる

## 5. 影響範囲

- 変更: `src-tauri/src/pdf/mod.rs`、`src-tauri/src/pdf/strip_audio.rs`（新規）、必要に応じて `src-tauri/tests/` に結合テスト
- 変更なし: `commands/pdf_convert.rs` のシグネチャ、TypeScript・Svelte 全般、`app.settings.json`、デザインアセット
- ドキュメント: スペック v4、imple・tasks・walk の新版（旧版は `archived/` へ）
