# seichi-portal-backend

整地鯖が公開しているフォームなどを内製化するために作成したリポジトリです。

## 機能について

| 機能名         | 詳細                                                                                                                                       |     | 
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | --- | 
| フォーム機能   | 整地鯖が提供する各種フォームを一元管理するシステム                                                                                         |     | 
| メッセージ機能 | フォーム等で回答した内容に対して連絡が必要な場合に利用できるシステム。<br><br>一般プレイヤー同士での連絡用に使用することは想定していない。 |     | 
| 情報確認機能   | 以下の情報が確認できる。<br>- フォーム回答履歴<br>- 処罰履歴<br>- お知らせ                                                                 |     | 


## APIについて

seichi-portal-backendではREST APIを用いて通信を行います。REST APIの定義は[OpenAPI v3.0.0](https://spec.openapis.org/oas/v3.0.0)を利用したものになっています。

[OpenAPI定義（APIドキュメント）](https://github.com/GiganticMinecraft/seichi-api-schema)

## プロジェクト俯瞰図

![image](./docs/overhead-view.drawio.svg)

## フロントエンド

フロントエンドは外部リポジトリになります。

[リポジトリ (seichi-portal-frontend)](https://github.com/GiganticMinecraft/seichi-portal-frontend)

## ライセンス

[Apache Licence 2.0](https://github.com/GiganticMinecraft/seichi-portal-backend/blob/master/LICENSE)