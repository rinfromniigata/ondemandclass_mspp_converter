# 修正記録（fix）: タイムアウト時に子孫プロセスごと終了する

- 区分: 軽微な修正（要件・設計・インターフェース・タスク構成・プランアセットの確定事項は変えない）
- 発見した作業: T4-5（`pdf/mod.rs` とコマンド `run_soffice_convert`）
- 関連: T2-1 の実施メモ「kill は直接の子プロセスのみで、孫プロセスは止めない。sofficeでの扱いは T4-5 で確認する」

---

## 1. 内容

### 1.1 `src-tauri/src/process.rs`
- タイムアウト時（と `try_wait` の失敗時）に、直接の子プロセスだけを `kill` していたのを、`kill_tree` で**子孫ごと終了**するように変えた
  - Windows: `taskkill /PID <pid> /T /F` を実行してから（コンソール窓は出さない）、念のため直接の子も `kill` する
  - Windows以外: これまでどおり直接の子だけを `kill` する
- `ProcessOutput::output_tail()` を追加した。stdout と stderr の末尾をつなげた文字列で、soffice が正常終了したのにPDFがない場合のエラー文言に使う
- 単体テスト `kills_grandchildren_on_timeout`（Windowsのみ）を追加した。`cmd /C ping …` を0.5秒で打ち切り、4秒以内に戻ることを確かめる
  - 孫（ping）が残るとパイプを読み切る待ち（5秒）まで戻らないため、戻るまでの時間で判定できる
  - `taskkill` を外すと5.5秒かかって失敗することを確認済み

### 1.2 `src-tauri/src/fs_util.rs`（新規）
- T4-3 で `audio/concat.rs` に置いた「一時フォルダの成果物を出力先へ移す（rename、できなければコピー）」処理を、PDF変換でも使うため `fs_util::move_or_copy` に移した
- `concat.rs` は `move_or_copy` を呼び、エラーの文言（「結合した音声を保存できません（…）」）はこれまでどおり呼び出し側で付ける。単体テストも `fs_util.rs` に移した

## 2. 理由

- T4-5 の確認で、`soffice.exe --headless --convert-to pdf` は実体の **`soffice.bin` を子プロセスとして起動し**、変換が終わるまで待つことがわかった（Win32_Process の親子関係で確認）
- 直接の子（`soffice.exe`）だけを kill すると、`soffice.bin` が変換を続けたまま残る。その間はアプリ専用プロファイルを握っているため、次の変換が残った `soffice.bin` に渡される。さらに、パイプを引き継いでいるので、戻るまでに5秒余計にかかる
- スペック5章の「タイムアウトは120秒とし、超過時はプロセスを終了して `Err`」を満たすには、子孫ごと終了する必要がある

## 3. 確認

- 実際の soffice で `run_with_timeout` を3秒で打ち切り、次の3点を確認した（一時的な結合テストで実施し、確認後に削除）
  - 3.3秒で戻る
  - `soffice.bin` が残らない
  - その直後の変換が成功する（プロファイルに `.lock` は残るが、次回の変換には影響しなかった）
- `cargo test`（単体112件・結合9件）、`cargo test --test concat_integration -- --ignored`（9件）がすべて成功し、`cargo clippy --all-targets` の警告は0、`cargo fmt --check` の差分もなし
