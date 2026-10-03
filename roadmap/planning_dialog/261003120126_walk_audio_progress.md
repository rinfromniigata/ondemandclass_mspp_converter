# 検証手順（walk）: 初期実装＋デザインシステム＋音声結合の進捗表示＋リリース

- 対象スペック: `roadmap/specs/ondemandclass_mspp_converter_SPEC_v3.md`
- 実装タスク: `roadmap/planning_dialog/261003120125_tasks_audio_progress.md`
- 前版: `roadmap/archived/261002133710_walk_sample_formats_and_release.md`（IDとチェック状態を引き継ぎ）
- 変更理由: `roadmap/development/261003115642_change_audio_progress.md`（前回: `261002133633_change_sample_formats_and_release.md`）

tasksの各Phaseが終わったら、同じ番号のPhaseを検証する。
（グローバルルール改訂 261002121815 の「Phaseの同期・アジャイル化」は、ユーザーの指示により本プロジェクトでは適用しない）
Phase 5・6はスペック13章の受け入れ基準の確認を兼ねる。
v2で追加した項目は W1-4・W5-4〜W5-10・W6-13。
261002の変更で内容を変えた項目は W3-2・W4-3・W6-1〜W6-10・W6-12、追加した項目は旧 W7-1〜W7-3（現 W8-1〜W8-3）。
v3（261003の変更）で追加した項目は W7-4〜W7-10。リリース準備の検証は W7-1〜W7-3 から W8-1〜W8-3 へ移動した（旧IDは欠番）。
**実ファイル（pptx/ppsx）を前提にする検証は、必ず両方の形式で行う。** 実ファイルの構成は imple 6.4（前編は pptx・ppsx の両方、後編は ppsx のみ。派生サンプルは `samples/derived/pptx/` と `samples/derived/ppsx/`）。

---

## Phase 1: 環境構築の検証
- [x] W1-1 【エージェント】`bun install` と `cargo build --manifest-path src-tauri/Cargo.toml` がエラーなく完了することを確認する
  - 実施メモ:
    - `bun install`: 終了コード0（変更なし）
    - `cargo build`: 終了コード0、エラーなし（初回2分06秒）。`src-tauri/target/debug/ondemandclass_mspp_converter.exe` が生成された
    - 警告1件: `warning: linker stdout: ライブラリ …ondemandclass_mspp_converter_lib.dll.lib とオブジェクト ….dll.exp を作成中`。MSVCリンカーの情報メッセージを、Rust 1.98で既定有効の `linker_messages` lintが警告として表示しているもの（テンプレートの `crate-type` に `cdylib` が含まれるため）。ビルド結果には影響しない
    - 申し送り: T6-1で `cargo clippy` を `-D warnings` で実行する場合、この警告で失敗する可能性がある。その時点で扱いを判断する
- [x] W1-2 【ユーザー】`bun run tauri dev` でウィンドウが開き、タイトルが「オンデマンドスライドコンバーター」であることを確認する
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W1-3 【エージェント】`git status` で `app.settings.json`・`samples/`・`node_modules/`・`src-tauri/target/` が追跡対象外であることを確認する
  - 実施メモ:
    - 4つとも実在する状態で `git status --short -uall` に現れないことを確認（コミット 0eadece 時点で作業ツリーはクリーン）
    - `git check-ignore -v` の該当ルール: `app.settings.json`・`samples/`・`node_modules` はルートの `.gitignore`、`src-tauri/target/` は `src-tauri/.gitignore` の `/target/`
    - `git ls-files` で、4つのパス配下に追跡済みファイルが0件であることも確認（過去に誤ってコミットされていない）
- [x] W1-4 【ユーザー】ウィンドウのタイトルバーとタスクバーに、アイコン原本から生成したアプリアイコンが表示されることを確認する
  - 依存: T1-10
  - 実施メモ: ユーザー報告により完了（Windowsで確認）。macOSでの見え方は W8-3（旧 W7-3）で確認する（261002 の変更で追加）

## Phase 2: Rust基盤の検証
- [x] W2-1 【エージェント】`cargo test` で process・settings の単体テストがすべて通ることを確認する
  - 実施メモ（261001）: `src-tauri` で `cargo test` を実行し、終了コード0。lib の単体テスト22件がすべて成功（失敗・ignore 0）。bin（main.rs）とdoctestは0件
    - process 6件: 正常終了・非ゼロ終了・タイムアウト（kill）・起動失敗・大量出力でのパイプ詰まりなし・stderr末尾の切り出し
    - settings 12件: parse 5件（全項目・省略時のデフォルト・不正JSON・必須パスの欠落／型不正・省略可能項目の不正値）、validate 3件（実在・パス不在／フォルダ指定・負の秒数）、load 3件（ファイルなし・BOM付き・検証失敗時に settings が null）、locate 1件
    - その他: error 2件・state 2件
    - 結合時に `linker_messages` の警告が1件出るが、T2-3の実施メモのとおりMSVCの情報メッセージで、テスト結果には影響しない
- [x] W2-2 【ユーザー】`app.settings.json` の `ffmpegPath` を存在しないパスに変えて `bun run tauri dev` を起動し、idle画面のBannerに「ffmpegPath を確認してください」を含むエラーが出て、ドロップを受け付けないことを確認する
  - 依存: T5-7（画面表示はPhase 5完了後に確認する）
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W2-3 【ユーザー】W2-2の状態からパスを正しく直し、「設定を再読み込み」ボタンでエラーが消えることを確認する
  - 依存: W2-2
  - 実施メモ: ユーザー報告により完了（問題なし）

## Phase 3: pptx解析の検証
- [x] W3-1 【エージェント】`cargo test` で pptx 解析の単体テスト・結合テストがすべて通ることを確認する
  - 受け入れ基準「sldIdLst順で表示順が確定」「p:timing順で結合」の確認を兼ねる
  - 実施メモ（261002）: `src-tauri` で `cargo test` を実行し、終了コード0、エラー・警告なし
    - lib の単体テスト120件すべて成功。うち pptx 47件（package 10・presentation 12・rels 7・slide 15・mod 3）
    - 結合テスト `tests/pptx_extract.rs` 9件すべて成功。受け入れ基準に当たるのは `display_order_follows_sld_id_list_not_file_numbers_or_rels`（sldIdLst順）と `two_audios_follow_timing_order_not_rels_or_tree_order`（p:timing順）。`ppsx_gives_same_result_as_pptx` も成功
    - `tests/concat_integration.rs` の9件は `#[ignore]`（ffmpeg が必要。W4-1 で実行する）
- [x] W3-2 【エージェント】`samples/` の実ファイル（前編のpptx・ppsx、後編のppsx）に対して `extract_pptx` 相当の処理を実行し（テスト用の `#[ignore]` テストまたはデバッグ出力）、スライド数・音声の有無・advTm が PowerPoint 上の内容と一致するかを実施メモに記録する
  - 依存: T1-8
  - 前編の pptx と ppsx で、結果（`SlideAudioMap` のJSON）が完全に一致することも確認する
  - 無音スライドの有無も記録する（W6-8・T6-2 の一部無音版の前提）
  - 実施メモ（261002）: `#[ignore]` 付きの一時的な結合テストで、3ファイルを `extract_from_file`（`extract_pptx` の本体）に通した。一時テストは確認後に削除した（T3-2〜T3-4 と同じ方法）
    - 結果（3ファイル共通）: 33枚、音声あり33枚、無音スライド0枚、1枚に音声が複数あるスライド0枚、リンク切れ0、動画0、警告なし、advTm 33件。スライドは sldIdLst 順に slide1〜33.xml、音声はスライド n が `ppt/media/media{n}.m4a`
    - advTm の範囲: 前編は7.492〜266.992秒、後編は8.001〜287.22秒
    - 前編の pptx と ppsx で、`SlideAudioMap` のJSON（整形後7,639バイト）がハッシュで完全に一致した
    - 所要時間（debugビルド）: 0.20〜0.36秒
    - PowerPoint 上の内容との照合: エージェントは PowerPoint を操作できないため、Rust実装とは別に、PowerShell（`System.IO.Compression` と正規表現）で zip 内のXMLを直接読んで照合した。3ファイルとも全33枚で次が一致した（不一致0）
      - `presentation.xml` の sldIdLst の件数と、rels で解決したスライドのパート名
      - 各スライドの advTm（`mc:AlternateContent` の両分岐の値が同じであることも確認）
      - `a:audioFile` の `r:link` から rels で解決したメディア名。外部リンク・`a:videoFile` はない
    - 補足: advTm は各スライドの音声の実際の長さ（ffprobe）より、前編は0.044〜0.608秒（平均0.185秒）、後編は0.033〜0.168秒（平均0.093秒）長い。「音声の再生が終わったら次のスライドへ進む」という PowerPoint の自動切り替え設定と合っている
    - 無音スライドがないため、W6-8 用の一部無音版は T6-2 で作る必要がある（T6-2 の記載どおり）
    - 照合の途中で一度、PowerShell と Rust のファイルの並び順の違いで別ファイルどうしを比べてしまい、見かけ上の不一致が出た。ファイル名の順序（Ordinal）で対応づけ直して再照合した（実装の問題ではない）

## Phase 4: 音声結合・PDF変換・JSON書き出しの検証
- [x] W4-1 【エージェント】`cargo test -- --ignored` で ffmpeg 結合テストがすべて通ることを確認する
  - 受け入れ基準「形式不一致時の再エンコード切り替え」の確認を兼ねる
  - 実施メモ（261002）: `app.settings.json` の ffmpeg/ffprobe（9.0.2 full_build、WinGet Links）を環境変数 `FFMPEG_PATH`・`FFPROBE_PATH` に設定し、`src-tauri` で `cargo test --test concat_integration -- --ignored` を実行した。9件すべて成功、終了コード0（5.5〜6.1秒、2回実行して2回とも成功）
    - 再エンコード切り替え: `reencodes_on_sample_rate_mismatch`・`reencodes_on_channel_mismatch`・`reencodes_when_mp3_is_mixed`・`mismatch_is_error_when_reencode_disabled`
    - copy結合: `copy_mode_with_silence_and_skipped_slides`・`copy_mode_keeps_mono`・`copy_mode_with_multichannel_silence`
    - エラー: `missing_media_is_error_with_slide_number`・`unreadable_media_is_error_with_slide_number`
    - 注意: 環境変数を設定せずに実行すると、テストはパスが分からず失敗する（スキップにはならない）
- [x] W4-2 【エージェント】W4-1の実行前後で、OSの一時フォルダに本アプリ由来の一時ディレクトリが残っていないことを確認する（成功ケース・失敗ケースの両方）
  - 実施メモ（261002）: 次の3つの方法で確認し、いずれも残りはなかった
    - テスト内の確認: `concat_integration` は tempfile の一時フォルダを `target/tmp/concat_integration/work` に差し替え、`run()` で `concat_audio` を呼ぶたびに work が空であることを確かめている。失敗ケース3件（再エンコード無効・メディアなし・読めないメディア。いずれも `run()` 経由）を含む9件が成功した。実行後の work と cases も0件
    - OSの一時フォルダ（`%LOCALAPPDATA%\Temp`）: W4-1の実行前後で、tempfile 既定の名前（`.tmp`＋ランダム文字）のフォルダは0件のまま
    - 前後の比較で `{GUID}.tmp`（0バイトのファイル）が4件増えていたため調べた。2回目の実行では、テスト中の増減は0件だった。一方、何もしていない40秒の間に1件増え、2件消えた。別の常駐プロセスが作ったり消したりしているもので、本アプリ由来ではないと判断した
    - 補足: OSの一時フォルダを実際に使う場合（差し替えなし）は、W4-3の PDF変換で `.tmp*` が前後とも0件であることを確認した
- [x] W4-3 【エージェント】`run_soffice_convert` を実ファイルの pptx と ppsx（前編）の両方で実行し、それぞれ `<basename>_slides.pdf` が生成され、入力と同じフォルダに置いた既存の `<basename>.pdf` が変更されていないことを確認する
  - 依存: T1-8
  - pptx と ppsx は basename が同じため、形式ごとに別の一時フォルダへコピーして実行する
  - スペック15章「ppsxがpptxと同様にPDFを出力する」の確認を兼ねる
  - 実施メモ（261002）: `#[ignore]` 付きの一時的な結合テストで、`run_soffice_convert` の本体 `convert_to_pdf` を呼んだ（コマンド自体は AppHandle が必要なため）。soffice は `app.settings.json` の LibreOffice、プロファイルはコマンドと同じ `%LOCALAPPDATA%\com.rinfromniigata.ondemandclass-mspp-converter\lo_profile`。一時テストと作業フォルダ（`target/tmp/w4_3/`）は確認後に削除した
    - 前編の pptx と ppsx を、`target/tmp/w4_3/pptx/` と `target/tmp/w4_3/ppsx/` にそれぞれコピーした。どちらにも内容の分かっている既存の `<basename>.pdf`（ダミー）を置いてから変換した
    - 両形式とも `<basename>_slides.pdf` が生成され、戻り値は出力パスと一致した。PDFは84,785,720バイト（両形式で同じ）で33ページ（`/Type /Page` と `/Count` がどちらも33。スライド数と一致）。ハッシュは異なる（生成時刻などの差と考えられる）
    - 既存の `<basename>.pdf` は、内容・更新日時とも変化なし。フォルダ内は入力・既存PDF・`_slides.pdf` の3ファイルだけ
    - 所要時間: pptx 21.6秒（1回目でプロファイル準備を含む）、ppsx 11.2秒。終了後に soffice のプロセスは残っていない
    - OSの一時フォルダの `.tmp*` は前後とも0件（W4-2の補足）

## Phase 5: フロントエンド・デザインシステムの検証
- [x] W5-1 【エージェント】`bun run check` と `bun run test` がエラーなく完了することを確認する
  - 実施メモ（261002）:
    - `bun run check`（svelte-kit sync → svelte-check）: 終了コード0。371ファイル、エラー0・警告0
    - `bun run test`（vitest 5.0.3）: 終了コード0。5ファイル55件すべて成功（約2.2秒）、警告・stderr出力なし
      - outputPaths 15件・confirmDialog 6件・orchestrator 6件・pipelineController 17件・audioConcatStep 11件（buildSegments 7件を含む）
- [x] W5-2 【ユーザー】アプリ起動直後にドロップゾーンのみが表示されることを確認する
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W5-3 【ユーザー】.pptx/.ppsx 以外のファイル、および2つ以上のファイルをドロップしたとき、idle画面のままBannerで理由が表示されることを確認する
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W5-4 【ユーザー】マウスを使わずに Tab / Shift+Tab / Enter / Space / Esc だけで、idle・結果・確認ダイアログの各画面のすべての操作ができ、フォーカス中の要素に枠線（フォーカスリング）がはっきり見えることを確認する
  - ライトモードのフォーカスリングは白背景に対し2.72:1のため、見えにくくないかを特に確認し、所感を実施メモに残す
  - 実施メモ: ユーザー報告により完了（問題なし）。ライトモードのフォーカスリングについても指摘なし（見えにくさの問題はないものとして扱う）
- [x] W5-5 【ユーザー】ウィンドウの端をドラッグして縮めたとき、480×360pxより小さくならず、どの画面も1カラムで崩れないことを確認する
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W5-6 【ユーザー】Windowsの設定で「アプリ モード」をダーク／ライトに切り替えると、アプリの配色が追従することを確認する
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W5-7 【ユーザー】Windowsの設定で「アニメーション効果」をオフにすると、画面遷移・ボタンの状態変化のアニメーションと処理中表示の回転が止まり、処理中であることがテキストで分かることを確認する
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W5-8 【エージェント】`bun scripts/contrast-check.ts` を実行し、ライト・ダーク両モードの文字色×背景色がすべて4.5:1以上であることを実施メモに記録する
  - 実施メモ（261003）: 終了コード0、「すべての文字色×背景色が 4.5:1 以上です」。結果は T5-11 の実施メモ（261002123436）と同じ
    - ライト（11組）: 最小は `--color-text-muted` × `--color-surface-sunken` の5.43:1。ほかは `--color-text-muted` × bg 5.80:1・× surface 6.19:1・× Banner背景 5.61:1、`--color-text-on-primary` × `--color-primary-strong` 5.78:1、それ以外は8.35:1以上
    - ダーク（11組）: 最小は `--color-text-muted` × Banner背景の6.37:1。それ以外は7.37:1以上
    - 参考（文字以外、3:1基準・判定対象外）: ライトは7組すべて3:1未満（1.52〜2.72:1。T5-11 の実施メモどおりの既知の値）、ダークは7組すべて3:1以上（4.09〜12.50:1）
    - OSに追従するダーク（`@media` 内）と `:root[data-theme="dark"]` の値の不一致はなし（不一致なら終了コード1になる）
- [x] W5-9 【ユーザー】ボタンにマウスを乗せる・押す・Tabで選ぶと、それぞれ薄い重ね色（状態レイヤー）で反応が分かることを確認する。ドラッグ中のドロップゾーンが浮き上がる（影が濃くなる）ことも確認する
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W5-10 【エージェント】DevTools（`bun run tauri dev` 時）で主要なクリック可能要素の寸法を確認し、すべて40×40px以上であることを実施メモに記録する
  - 実施メモ（261003）: 環境変数 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` を付けて `bun run tauri dev` を起動した。WebView2 に DevTools と同じプロトコル（CDP）で接続し、スクラッチ領域の計測スクリプト（リポジトリ外）で `getBoundingClientRect` を取得した。計測後にアプリと開発サーバーを終了した
    - 対象: `button`・`a[href]`・`input`・`[role=button]`・`tabindex` 付き要素などで、表示中のもの。`/dev/screens` の画面切り替え用のナビゲーションは開発専用のため除いた
    - 画面: idle（設定エラー時。ストア `settingsStatus` に疑似エラーを入れて表示）と、`/dev/screens` の processing-extract・processing・done・partial・failed・dialog
    - ウィンドウ: 既定の720×520と、最小の480×360（`Emulation.setDeviceMetricsOverride`）。どちらも同じ結果（devicePixelRatio 1.5、寸法はCSS px）
    - 結果: すべて40×40px以上（NG 0件）。最小は幅73.9px（「ログ詳細」）、高さ40px（全ボタン。Button の `min-height: 40px`）
      - 設定を再読み込み 130.3×40
      - 出力フォルダを開く 180.5×40、別のファイルを変換する 176.1×40、ログ詳細 73.9×40
      - 確認ダイアログ: キャンセル 79.1×40、上書きする 109.4×40
    - 通常の idle 画面と処理中画面には、クリックできる要素がない（ドロップゾーンはドラッグ＆ドロップ専用）

## Phase 6: 受け入れ検証（実ファイル）
- [x] W6-1 【ユーザー】`samples/derived/pptx/original.pptx` と `samples/derived/ppsx/original.ppsx` をそれぞれドロップし、どちらもログが「pptx解析 → 音声結合／PDF変換（到着順）→ タイムスタンプ書き出し」の順に表示され、3ファイルが元ファイルと同じフォルダに生成されることを確認する
  - 依存: T6-2
  - pptx解析中は画面中央に処理中表示が1つ、音声結合とPDF変換は各ログ行に処理中表示が出ることも確認する
  - 実施メモ: ユーザー報告により完了（問題なし）。成果物は pptx 版 09:45、ppsx 版 09:48（261003）に生成
- [x] W6-2 【ユーザー】W6-1で生成された `_audio.m4a` を再生し、`_timestamps.json` の各スライドの開始秒でスライドの話題が切り替わっていることを数か所で確認する（pptx・ppsxの両方）
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W6-3 【ユーザー】W6-1で生成された `_slides.pdf` を開き、スライドの見た目（フォント・図形）とテキスト選択ができることを確認する（pptx・ppsxの両方）
  - 実施メモ: ユーザー報告により完了（問題なし）
- [x] W6-4 【エージェント】W6-1の pptx 版と ppsx 版の成果物を比べ、`_timestamps.json` の内容が一致し、`_audio.m4a` の長さと `_slides.pdf` のページ数が同じであることを確認する
  - 旧内容（ppsx版をドロップして3ファイルの生成を確認する）は W6-1 に含めた
  - 実施メモ（実行結果 261003112956）: 合格
    - `original_timestamps.json`: 両形式でバイト単位で一致（2,432バイト、33スライド、最終 `endSec` 3107.317）
    - `original_audio.m4a`: 両形式でSHA-256まで一致（82,399,622バイト）。ffprobe で aac／44100Hz／2ch、長さ 3107.316984秒。タイムスタンプの最終 `endSec` とも一致する
    - `original_slides.pdf`: 両形式とも33ページ（ページツリーの `/Count 33`、ページオブジェクト33個）、84,785,720バイト
    - 参考: PDFはサイズが同じでもバイト列は一致しない（作成日時 `/CreationDate` などが異なる）。判定基準はページ数のため合否に影響しない。見た目は W6-3 でユーザーが確認済み
- [ ] W6-5 【ユーザー】リンク切れ版（`broken_link`）を pptx・ppsx の両方でドロップし、処理が止まらず、該当スライド番号を含む警告がBannerで表示され、3ファイルが生成されることを確認する
  - 依存: T6-2
  - ログ行には「完了（警告あり）」のラベルが出ることも確認する
- [ ] W6-6 【ユーザー】音声形式不一致版（`format_mismatch`）を pptx・ppsx の両方でドロップし、再エンコードした旨の警告がBannerで表示され、音声が途中で壊れずに再生できることを確認する
  - 依存: T6-2
- [ ] W6-7 【ユーザー】音声なし版（`no_audio`）を pptx・ppsx の両方でドロップし、音声結合・タイムスタンプ書き出しが理由付きで失敗表示され、PDFだけが生成され、状態表示が「一部エラー」になることを確認する
  - 依存: T6-2
- [ ] W6-8 【ユーザー】一部無音版（`partial_silence`）を pptx・ppsx の両方で、`silentSlideHandling` を `"insert_silence"` と `"skip"` に切り替えて変換し、`"skip"` では無音区間が詰められ、JSONで該当スライドが長さ0になることを確認する
  - 依存: T6-2
  - 設定を変えたら「設定を再読み込み」を押す。2回目以降の変換は上書き確認で「上書きする」を選ぶ
- [ ] W6-9 【ユーザー】W6-1と同じファイル（pptx・ppsxの両方）をもう一度ドロップし、処理開始前にアプリ内の確認ダイアログ（既存ファイル名の一覧、「上書きする」「キャンセル」）が出ること、キャンセル（ボタンとEscの両方）すると既存ファイルの更新日時が変わらないことを確認する
- [ ] W6-10 【ユーザー】pptx・ppsx それぞれの変換結果の画面で「出力フォルダを開く」を押し、エクスプローラーが開き、成果物が選択表示されることを確認する
- [ ] W6-11 【エージェント】コードを検索し、フォルダ監視・スケジューラ・ネットワーク通信（Webフォントの読み込みを含む）など、ドラッグ＆ドロップ以外のトリガーや外部通信が実装されていないことを確認する
- [ ] W6-12 【ユーザー】リリースビルドの実行ファイルを、同じフォルダに `app.settings.json` を置いた状態で起動し、W6-1と同様に pptx・ppsx の両方を変換できることを確認する
  - 依存: T6-4
  - W6-1 の成果物が残っている場合は、上書き確認で「上書きする」を選んでよい
- [ ] W6-13 【エージェント】コードを検索し、コンポーネント内で色・影・角丸・余白が直接の値（`#xxxxxx`、`rgba(...)`、`px` の角丸・影等）で書かれておらず、`tokens.css` のトークン経由になっていることを確認する。また `src/lib/styles/tokens.css` の値が原本と一致することを差分で確認する

## Phase 7: 音声結合の進捗表示の検証
- W7-1〜W7-3 → Phase 8 の W8-1〜W8-3 へ移動（`261003115642_change_audio_progress.md`）
- [ ] W7-4 【エージェント】`cargo test`・`bun run check`・`bun run test` がエラーなく完了することを確認する
  - 依存: T7-6, T7-7, T7-9, T7-10
  - process（`run_with_timeout_streaming`）・progress の単体テスト、TypeScript の進捗関連テスト（imple 6.1・6.2）の件数を実施メモに残す
- [ ] W7-5 【エージェント】`cargo test -- --ignored`（ffmpeg の環境変数つき）で、結合テストの全ケースが通り、通知された割合が 0〜1 に収まって単調に増え、成功時は最後が 1、失敗時は 1 が送られないことを確認する
  - 依存: T7-8
  - copy・再エンコード・copy失敗からの再試行の各方式を含める
- [ ] W7-6 【ユーザー】`bun run tauri dev` で `samples/derived/pptx/original.pptx` と `samples/derived/ppsx/original.ppsx` をそれぞれドロップし、音声結合の行だけにバーと割合（%）が表示され、0% から 100% へ戻らずに進むこと、PDF変換の行は今までどおり回転表示であること、全体の進捗や「1/4」のような段階表示がないことを確認する
  - 依存: T7-10
  - 成果物が残っている場合は、上書き確認で「上書きする」を選んでよい
- [ ] W7-7 【ユーザー】音声形式不一致版（`format_mismatch`）を pptx・ppsx の両方でドロップし、再エンコードでも割合が戻らずに 100% まで進み、完了後は今までどおり再エンコードの警告が出ることを確認する
  - 依存: T7-10
- [ ] W7-8 【ユーザー】Windowsの設定で「アニメーション効果」をオフにして W7-6 と同じ操作をし、バーが伸びる動きなしで値だけ更新され、割合のテキストで進み具合が分かることを確認する
  - 依存: W7-6
- [ ] W7-9 【エージェント】変更・追加したコンポーネント（`ProgressIndicator`・`ListItem`・`StepLog`・`ProcessingView`）に、色・影・角丸・余白の直接の値がなく、トークン経由になっていることを確認する。確定型の割合テキストの色×背景色のコントラストが既存の組み合わせ（`bun scripts/contrast-check.ts`）に含まれていることも確認する
  - 依存: T7-10
- [ ] W7-10 【ユーザー】T7-11 で作り直したリリースビルドの実行ファイルを、同じフォルダに `app.settings.json` を置いた状態で起動し、pptx・ppsx の両方で音声結合の進捗表示が W7-6 と同じように出て、3ファイルが生成されることを確認する
  - 依存: T7-11

## Phase 8: リリース準備の検証
- [ ] W8-1 【エージェント】`.github/workflows/release.yml` を確認し、`v*` タグで起動すること、Windows x64 と macOS Universal のビルドがあること、ドラフトで作成すること、アイコンを生成する手順がないことを実施メモに記録する
  - 依存: T8-2
  - 旧ID: W7-1
- [ ] W8-2 【ユーザー】タグのpush後、GitHub Actions が成功し、ドラフトリリースに Windows x64 と macOS Universal の成果物が付いていることを確認する
  - 依存: T8-4
  - 旧ID: W7-2
- [ ] W8-3 【ユーザー】ドラフトリリースの macOS 版を macOS で開き、Dock・Finder でアプリアイコンが判別できることを確認する（Windows は W1-4 で確認済み）
  - 依存: T8-4
  - macOS 上での変換動作は確認の対象外（imple 9章）
  - 旧ID: W7-3
