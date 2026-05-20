# seichi-portal-backend

このリポジトリは Seichi Portal のバックエンド実装です。

プロジェクトの目的やフロントエンドなどの関連リポジトリについては、[こちらのリポジトリ](https://github.com/GiganticMinecraft/seichi-portal)を参照してください。

## 主な機能

| 機能名 | 詳細 |
| --- | --- |
| フォーム機能 | 整地鯖が提供する各種フォームを一元管理する |
| メッセージ機能 | ユーザーから送られてきたフォームのリクエストに対して運営からメッセージを送信できる |
| 情報確認機能 | フォーム回答履歴、処罰履歴、お知らせ情報が確認できる |

## API定義

Seichi Portal ではフロントエンドとバックエンド間の通信に REST API を使っており、API のスキーマは [OpenAPI v3.0.0](https://spec.openapis.org/oas/v3.0.0) ベースの [`docs/openapi.json`](./docs/openapi.json) を正本として管理しています。

API 定義は `utoipa` から生成されるため、ハンドラやスキーマを変更したときは以下を実行して生成物もコミットしてください。

```bash
makers generate-openapi
```

CI では同じ生成処理を実行し、`docs/openapi.json` に差分が残っていないことを確認します。

## 開発環境とミドルウェア

バックエンド言語には Rust を採用しており、MariaDB にフォームなどの必要な情報が永続化されます。開発環境では Docker Compose を使うため、必要であれば別途導入が必要です。
セットアップ手順と `sqlx` のオフライン検証手順は [CONTRIBUTING.md](./CONTRIBUTING.md) を参照してください。

## プロジェクト俯瞰図

![image](./docs/overhead-view.drawio.svg)

## ライセンス

[Apache Licence 2.0](https://github.com/GiganticMinecraft/seichi-portal-backend/blob/main/LICENSE)
