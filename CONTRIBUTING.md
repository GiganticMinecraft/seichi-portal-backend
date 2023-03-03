## 開発環境のセットアップ方法

### ツール類のインストール

- `rustup` で Rust ツールチェインをインストールする

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

- `cargo-make`と `sea-orm-cli` を `cargo` でインストールする

```shell
cargo install cargo-make sea-orm-cli
```

### ローカルで起動する

データベース周りの接続情報は [.env.example](./server/.env.example) にまとまっており、 DB を起動するためには `.env` ファイルが必要なため、以下のようにファイルをコピーします。

データーベースとサーバーを起動するには `up` タスクを実行します。
```shell
seichi-portal-backend> makers up 
```

## 付録 A: cargo make のタスク一覧

ワークスペース内で `cargo make {タスク}` または `makers {タスク}` を実行することで `Makefile.toml` に書かれたタスクを実行することができます。

### cargo ワークスペースで共通のタスク

ワークスペースで共通のタスクはワークスペースのトップディレクトリで実行すると、すべてのクレートに対してタスクが実行されます。
各クレートのディレクトリのトップで実行すると各クレートに対してタスクが実行されます。

| タスク名 |                実行されるタスク                 | 備考                                                           |
| :------: | :---------------------------------------------: | :------------------------------------------------------------- |
|   fix    | cargo clippy --fix --allow-dirty --allow-staged | clippy が自動でコードを修正します                              |
|   test   |                cargo nextest run                | nextest によるテストの実行を行います                           |
|   lint   |           cargo clippy -- -D warnings           | clippy によるコードチェックを行います                          |
|  format  |                    cargo fmt                    | rustfmt によるコード整形を行います                             |
|  pretty  | fix -> test -> lint -> format の順に実行します  | 上記 4 つをすべて実行します、push の前に行うことが推奨されます |
