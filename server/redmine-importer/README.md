# redmine-importer

Redmine の issue を Portal のフォーム回答へ移行する、一回限りの Job 用 crate です。
Redmine は API の GET だけで読み取り、Portal への保存は backend の Domain / Usecase /
Repository 境界を通します。通常の回答投稿で発生する通知や Discord webhook は発生させません。

環境変数は次のとおりです。

- `REDMINE_BASE_URL`: Redmine の URL
- `REDMINE_API_KEY`: Readonly API key
- `MYSQL_DATABASE`, `MYSQL_USER`, `MYSQL_PASSWORD`, `MYSQL_HOST`, `MYSQL_PORT`: Portal DB

設定ファイルは `REDMINE_IMPORT_CONFIG` で指定できます。未指定時はリポジトリ内の
`server/redmine-importer/config/redmine-import.json`、Docker Image では
`/etc/seichi-portal/redmine-import.json` を読み込みます。

tracker mapping は Redmine tracker ID、期待する tracker 名、Portal form UUID、期待する
form title、質問ごとの値の生成方法、付与する answer label を持ちます。tracker ID と form
UUID を使うため、表示名の偶然の一致で別のフォームへ保存しません。`subject_and_description`
は Redmine の subject と description を一つの回答値へ結合し、`static` は選択肢などを
設定ファイルで明示します。必須質問の mapping が存在しない場合や選択肢が合わない場合は
推測せず失敗します。

各 tracker mapping の `custom_field_content_template_key` は、値を追記する本文質問を
指定します。`custom_field_question_mappings` で個別に移せる custom field を指定した場合は
その質問へ保存し、それ以外の non-empty な custom field は本文へ次の形式で追記します。

```text
---
Redmine カスタムフィールド:
- 連絡先 (ID: 1): example@example.com
```

現在は `通報` tracker の `違反者ID` を `通報フォーム` の `target_minecraft_id` へ移します。
`公共建築`、`修繕依頼`、`不要保護報告` は案件の性質に合わせて別フォームへ分け、終了条件、
対象サーバー・ワールド・座標、修繕内容、不要と判断した理由などの custom field を対応する
質問へ移します。その他の custom field は各フォームの本文へ残します。Redmine の null、
空文字、空配列は保存対象になりません。

Redmine の `連絡先` または `ID` custom field に有効な値がある場合は、Redmine author の
表示名をその値で補正します。`ID` と `連絡先` の両方があれば両方を表示し、Bot アカウント
から登録された issue でも同じ補正を適用します。Redmine の user ID 自体は保持します。

`お問い合わせ` tracker は、件名に `Mod の使用可否`、ログイン不能、処罰への異議申し立て
（または BAN 解除申立）が明記されている場合に、それぞれの専用フォームへ移します。件名に
カテゴリがない場合でも、件名または本文で `経験値` と `オーバーフロー` が近接して現れる
場合だけ経験値フォームへ移します。一般的なキーワードだけの曖昧な問い合わせは、専用フォーム
へ推測で移さず `お問い合わせフォーム` に残します。専用フォームでも件名・本文は内容質問へ
保存し、必要な Mod 名や URL が原文から特定できない場合は、その旨を示す値を設定します。

`status_label_mappings` は tracker と Redmine status の組み合わせに追加する answer label
を指定します。現在、`アイデア提案` tracker の `承認` と `却下` にそれぞれ状態ラベルを
付けます。両方とも Portal の answer status は `COMPLETED` に変換されます。

移行対象の Redmine project は `redmine-import.json` の `redmine_projects` に ID と期待する
名前を列挙します。複数 project を一度の実行で取得し、project ID は重複を許可しません。
親 project を指定した場合も子 project の issue は自動では含めず、移行する子 project は個別に
列挙します。`REDMINE_PROJECT_ID` 環境変数は使用しません。
issue 詳細の取得は `detail_concurrency` 件まで並列化します。Redmine の負荷やレート制限に
応じて設定値を調整できます。

現在の設定では `アイデア提案` tracker（ID 19）の回答だけを `PUBLIC`、その他の対象
tracker の回答を `PRIVATE` として作成します。対象外 tracker は設定で固定され、移行しません。

実行は次の順序で行います。

```bash
cargo run -p redmine-importer -- plan
cargo run -p redmine-importer -- verify
cargo run -p redmine-importer -- import
```

`plan` は Redmine の全対象 issue を取得し、フォーム・質問・status・label と既存回答を
照合します。`verify` は同じ照合を行い、`import` は issue ごとに回答・Redmine 参照・
journal コメント・label を一つの transaction で保存します。同じ issue の同一 payload は
再実行時にスキップし、異なる payload が既に保存されている場合は停止します。

issue 一覧の `relations` も実行全体で重複除去し、両端が移行対象 tracker の関連だけを、全 issue
の保存後に `answer_relations` へ保存します。Portal の関連は対称なモデルであるため Redmine の
relation type は保持しません。対象外 tracker や設定外 project への関連は警告してスキップし、
関連保存も同じ入力で再実行できます。

添付ファイルと notes が空の journal は Portal の回答項目へは保存しません。添付ファイルは
警告として出力され、空の journal はコメントとしてスキップされます。Redmine の custom
field は、設定された個別質問または本文へ保存します。
Portal の既存 timestamp 列が秒精度のため、Redmine の日時に含まれる小数秒は切り捨てます。
