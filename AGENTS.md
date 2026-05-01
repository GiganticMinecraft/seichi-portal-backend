# seichi-portal-backend AI エージェント向けドキュメント

## 概要

seichi-portal-backend は、公開 Minecraft サーバーのポータルサイトのバックエンド API を提供するためのプロジェクトです。

## 検証について

### コンパイル

```bash
cd server/ && cargo build
```

### lint, フォーマット

```bash
cd server/ && makers pretty
```

### `sqlx` メタデータ

今後 typed query (`query!`, `query_as!` など) を追加・変更する PR では、ルート `.env` の `DATABASE_URL` を使って `cd server/ && cargo sqlx prepare --workspace` を実行し、`.sqlx/` の更新をコミットに含めること。

## ローカル開発時の認証

`docker compose up` で Valkey にデバッグ用セッションデータが自動投入される。API リクエスト時は以下のセッション ID を `Authorization` ヘッダーに指定する:

- `Bearer debug_session` — ADMINISTRATOR ロール
- `Bearer debug_session_standard` — STANDARD_USER ロール
