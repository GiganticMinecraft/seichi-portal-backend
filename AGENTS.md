# seichi-portal-backend AI エージェント向けドキュメント

## 概要

seichi-portal-backend は、公開 Minecraft サーバーのポータルサイトのバックエンド API を提供するためのプロジェクトです。

## 検証について

### コンパイル

```bash
cargo build
```

### lint, フォーマット

```bash
makers pretty
```

### `sqlx` メタデータ

今後 typed query (`query!`, `query_as!` など) を追加・変更する PR では、ルート `.env` の `DATABASE_URL` を使って `cargo sqlx prepare --workspace` を実行し、`.sqlx/` の更新をコミットに含めること。

## AI Agent 向けスキル

このリポジトリには、実装時に守るべき規約をまとめた skill を `.agents/skills/seichi-portal-backend-coding-discipline` に置いている。

AI Agent が skill を読み込める場合は、Rust コード、`sqlx` クエリ、Repository、Usecase、認可境界を変更する前にこの skill を読むこと。

Claude Code 向けには `.claude` を `.agents` へのシンボリックリンクにしている。

## ローカル開発時の認証

`docker compose up` で Valkey にデバッグ用セッションデータが自動投入される。API リクエスト時は以下のセッション ID を `Authorization` ヘッダーに指定する:

- `Bearer debug_session` — ADMINISTRATOR ロール
- `Bearer debug_session_standard` — STANDARD_USER ロール
