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

データベース周りの接続情報は [.env.example](./server/.env.example) にまとまっており、 DB を起動するためには `.env` ファイルが必要なため、ファイルをコピーします。
必要に応じて値を書きかえてください。

データーベースとサーバーを起動するにはリポジトリのディレクトリトップで `up` タスクを実行します。

```shell
seichi-portal-backend> makers up
```

## 付録 A: cargo make のタスク一覧

ワークスペース内で `cargo make {タスク}` または `makers {タスク}` を実行することで `Makefile.toml` に書かれたタスクを実行することができます。

### リポジトリのディレクトリトップで使えるタスク

|  タスク名  |    実行されるタスク    | 備考                                |
| :--------: | :--------------------: | :---------------------------------- |
|   up-db    |  docker compose up -d  | データベースを立ち上げます          |
| run-server | cd server && cargo run | server/app をビルドして立ち上げます |
|     up     |  up-db -> run-server   | 上記 2 つを順番にやってくれます     |

### cargo ワークスペース共通で使えるタスク

cargo ワークスペースで共通のタスクはワークスペースのトップディレクトリで実行すると、すべてのクレートに対してタスクが実行されます。
各クレートのディレクトリのトップで実行すると各クレートに対してタスクが実行されます。

| タスク名 |                実行されるタスク                 | 備考                                                           |
| :------: | :---------------------------------------------: | :------------------------------------------------------------- |
|   fix    | cargo clippy --fix --allow-dirty --allow-staged | clippy が自動でコードを修正します                              |
|   test   |                cargo nextest run                | nextest によるテストの実行を行います                           |
|   lint   |           cargo clippy -- -D warnings           | clippy によるコードチェックを行います                          |
|  format  |                    cargo fmt                    | rustfmt によるコード整形を行います                             |
|  pretty  | fix -> test -> lint -> format の順に実行します  | 上記 4 つをすべて実行します、push の前に行うことが推奨されます |
