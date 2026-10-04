# fix: デザインアセットの原本を specs/ に置き直す

- 種別: 軽微な修正（要件・設計・インターフェース・タスク構成・確定事項のいずれも変えない）
- 日時: 261004114620
- 関連: T1-9 の実施メモ「要確認（プランアセットの置き場所）」、W6-13

## 内容

`roadmap/specs/` に、スペックシート v4 と同じ版番号でデザインアセット2点を置いた。

- `ondemandclass_mspp_converter_tokens_v4.css`（5,179バイト）
- `ondemandclass_mspp_converter_brand_v4.md`（1,892バイト）

## 理由

T1-9 でデザインアセットをリポジトリ直下から `src/`・`assets/design/` へ移した結果、原本が `specs/` になく、git履歴（コミット 5c87d89 の親）にしか残っていなかった。グローバルルールでは、値の正本であるデザインアセットを `specs/` に版番号付きで置く。W6-13 の差分確認も、履歴から原本を取り出して行う必要があった。

## 内容の出どころと確認

- tokens: git履歴（5c87d89 の親）の原本。`src/lib/styles/tokens.css` から1行目（原本名の説明コメント）を除いた内容と `diff` で差分なし（SHA-256 `fca4341dc36a5c12…`）
- brand: git履歴（5c87d89 の親）の原本。`assets/design/ondemandclass_mspp_converter_brand.md` と `cmp` で一致（SHA-256 `829993a2f74a97fa…`）
- 値は変更していない。`src/`・`assets/design/` の配置物は変更していない

## 未対応

- `ondemandclass_mspp_converter_icon_master_v4.svg` は置いていない（今回の指示は tokens と brand のみ）。`specs/` を1つの版でそろえるには、マスターSVGも置く必要がある。リリース準備（アイコン一式がマスターSVGから生成されたものであることの確認）でも参照するため、ユーザーの判断を仰ぐ
