# 変更履歴

このプロジェクトの主な変更をこのファイルに記録します。

形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に、バージョン番号は [セマンティック バージョニング](https://semver.org/lang/ja/) に従います。

## [Unreleased]

## [0.1.0] - 2026-10-04

最初のリリースです。

### Added

- `.pptx` / `.ppsx` ファイルをウィンドウにドラッグ＆ドロップして変換を始める機能
- 入力ファイルと同じフォルダに、次の3つのファイルを生成する機能
  - `<basename>_audio.m4a`: 各スライドのナレーションをスライド順に結合した音声
  - `<basename>_slides.pdf`: LibreOffice で書き出したテキスト層付きの PDF
  - `<basename>_timestamps.json`: 結合音声の中での各スライドの開始・終了時刻
- スライド順を `presentation.xml` の表示順で決め、1スライド内の複数の音声を再生順に結合する処理
- スライド間で音声の形式（コーデック・サンプルレート・チャンネル数）が異なる場合に、再エンコードで結合する機能（`audioReencodeOnMismatch` で切り替え）
- ナレーションのないスライドの扱いを選ぶ設定（`silentSlideHandling`: 無音を挿入する／省く、`silentSlideDefaultSec`: 挿入する無音の秒数）
- 音声のリンク切れ・動画として埋め込まれたナレーションを検出したときに、処理を止めずに警告を表示する機能
- 音声の結合中に進み具合を割合（%）で表示する機能
- PDF への変換前に PowerPoint の音声の再生アイコンを除去し、PDF に再生アイコン・音声データを含めない処理（入力ファイルは変更しない）
- 出力先に同じ名前のファイルがあるときに、処理の前に上書きを確認するダイアログ
- 外部ソフト（ffmpeg・ffprobe・LibreOffice）のパスを `app.settings.json` で指定し、起動時に検証して誤りを表示する機能
- 完了画面での生成ファイルの一覧・警告・処理ログの表示と、出力フォルダを開く機能
- Material Design 3 を土台にしたデザインシステム（ライト・ダークモードへの追従、キーボード操作、アニメーションを減らす設定への対応）
- アプリアイコン
- GitHub 用の README と Apache License 2.0 のライセンス表記

[Unreleased]: https://github.com/rinfromniigata/ondemandclass_mspp_converter/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rinfromniigata/ondemandclass_mspp_converter/releases/tag/v0.1.0
