# seichi-portal-search-system-design-doc

## 概要

### 達成したい内容 + その目的

#### 機能要件

- seichi-portal が管理するデータの一部(フォーム、フォームへの回答、回答に対するコメント、ラベル、ユーザー名、ユーザーID)
  に対する全文検索ができる
- 検索が可能な上記データを蓄積する MariaDB のデータを全文検索エンジンのデータとして同期すること
- キーワード単位でのフィルタ(AND OR NOT が使えれば十分)検索ができること

#### 非機能要件

- 機能要件を満たすクエリによる検索結果を遅くとも1秒以内に取得できること
    - ここで指定されるクエリは、同時に使用されるフィルタが4つ以下かつ、検索文字列長が10未満であることを想定している
    - [現状のアイデア提案フォームの回答データ](https://redmine.seichi.click/projects/idea/issues)
      を参照すると、現在のデータ総数が約14000件あり、過去のデータを削除するという運用を行わない限り減少しない
- MariaDB に行われた変更(データの追加を含む)を遅くとも1秒以内に反映できる
    - データの変更が行われる頻度はフォームへの回答・コメント・管理操作時のみであり、多くても1日あたり数100回以内で収まるものと推定される

### 行わないこと

- 検索結果へのアクセス制御(これは検索エンジン側ではなく検索を実行する API の実装側で制御をする)

## 構成の全体像

![overview](./search-system-overview.drawio.svg)

## ミドルウェア選定における選択肢の比較検討

### CDC ツール

| ツール名                                               | メリット                                               | デメリット                                 |
|----------------------------------------------------|----------------------------------------------------|---------------------------------------|
| [Debezium](https://debezium.io/)                   | 機能が豊富で拡張性が高い、公式での MariaDB サポートがある、大量のデータを効率的に処理できる | 設定が複雑                                 |
| [Meilisync](https://github.com/long2ice/meilisync) | Meilisearch と RDB を同期することに特化                       | 安定していない                               |
| [Maxwell](https://github.com/zendesk/maxwell)      | Debezium と比べるとシンプルで軽量                              | MySQL に特化、Debezium と比べると処理できるデータ量が少ない |

### メッセージブローカー

| ツール名                                                                   | メリット                                                   | デメリット                                                             |
|------------------------------------------------------------------------|--------------------------------------------------------|-------------------------------------------------------------------|
| [RabbitMQ](https://www.rabbitmq.com/)                                  | Kafka と比べ軽量、メッセージルーティングが柔軟、メッセージの受信が保証される、メッセージの永続化が可能 | 大規模な分散処理には不向き、Kafka と比べると送信できるデータ数が少ない(毎秒数千程度)                    |
| [Kafka](https://kafka.apache.org/)                                     | 1秒あたり最大数百万の処理が可能、大規模な分散処理に向いている                        | RabbitMQ と比べ複雑で運用コストが高い                                           |
| [Redis Pub/Sub](https://redis.io/docs/latest/develop/interact/pubsub/) | RabbitMQ・Kafka と比べ非常に軽量でリアルタイム処理に向いている                 | 標準機能でメッセージを永続化できない、メッセージの受信が保証されない、Subscriber がいない状態でメッセージが保持されない |

### 全文検索エンジン

| エンジン名                                                          | メリット                                      | デメリット                                          |
|----------------------------------------------------------------|-------------------------------------------|------------------------------------------------|
| [Meilisearch](https://www.meilisearch.com/)                    | 日本語の全文検索に対応、最大50ms以内に結果が取得できる、誤字に強い       | セルフホストの Meilisearch では複数ノードでの処理や大規模データには向いていない |
| [Elasticsearch](https://www.elastic.co/jp/elasticsearch)       | 分散処理が可能で大規模データの検索に向いている、複雑なクエリに対応し分析機能がある | Meilisearch ほど高速ではない、複雑で運用コストが高い               |
| [MariaDB Mroonga](https://mariadb.com/kb/en/mroonga-overview/) | 他の全文検索エンジンを必要としないため構成が単純になる               | 検索時のフィルタリングを SQL として構成する必要がある                  |

## 構成要素と選定理由

| 構成          | 選定理由                                                                                                                                         |
|-------------|----------------------------------------------------------------------------------------------------------------------------------------------|
| Debezium    | 安定しているかつ、[公式として MariaDB をサポートしている](https://debezium.io/documentation/reference/stable/connectors/mariadb.html)からで、MariaDB の独自機能を使う選択肢を潰さないため |
| RabbitMQ    | メッセージの受信を保証する機能が機能要件を満たすのに十分で、Kafka ほどの規模を必要としていないため                                                                                         |
| Meilisearch | 機能・非機能要件を十分に満たし、誤字に対する対応が強力でユーザー目線でも使いやすいため                                                                                                  |

## 各コンポーネントにおけるデータの永続性

### Debezium

Debezium は MariaDB の binlog を監視するために必要な offset やスキーマ履歴を管理するファイルの消失リスクがある。
しかし、これらのデータが消失したとしても、binlog が保持されている限りは Debezium が異常終了したとしても大きな問題にはならない。
**ただし、binlog の管理をする [MariaDB の設定](https://mariadb.com/kb/en/using-and-maintaining-the-binary-log/)
として `expire_logs_days` や `max_binlog_total_size` が設定されている場合は注意が必要である**(
デフォルトではどちらも無効化されている)。
もし同期が完了していない状態で binlog が消失した場合は再同期を行う必要があり、この機能は seichi-portal-backend が持つ予定である。

MariaDB Connector
用の障害対応用ドキュメントが見当たらないため、ほぼ同じ仕組みであろう [MySQL コネクター用のページ](https://docs.redhat.com/ja/documentation/red_hat_integration/2022.q1/html/debezium_user_guide/how-debezium-mysql-connectors-handle-faults-and-problems#how-debezium-mysql-connectors-handle-faults-and-problems)
を参照すると、

> 障害が発生しても、システムはイベントを失いません。ただし、障害から復旧している間は、変更イベントが繰り返えされる可能性があります。このような正常でない状態では、Debezium
> は Kafka と同様に、変更イベントを 少なくとも 1 回 配信します。

とあり、同一イベントが複数配信される可能性があることがわかる。
しかし、Consumer(seichi-portal-backend) の変更イベント受信時の処理が冪等であれば良く、すでにそのような実装が行われているので問題になることはない。

### RabbitMQ

RabbitMQ の[永続化キュー](https://www.rabbitmq.com/docs/queues#durability)を使用すれば、キューとメッセージが永続化される。
例えば以下のような設定ファイルを使用することで永続化キューを使用することができる。

```json
  "queues": [
  {
    "name": "seichi_portal",
    "vhost": "/",
    "durable": true,
    "auto_delete": false,
    "internal": false,
    "arguments": {}
  }
]
```

Consumer(seichi-portal-backend) が ack を送信しない限りは、RabbitMQ が異常終了したとしてもメッセージ消失の可能性はない。
しかし、ack 送信前に RabbitMQ が異常終了した場合は Consumer(seichi-portal-backend)
がメッセージを受信したときの処理が冪等であれば対応可能で、すでにそのような実装が行われているので問題になることはない。

### Meilisearch

Meilisearch は標準でデータをストレージ上に永続化するため、Meilisearch がクラッシュしたとしてもデータは保持される。しかし、仮に
Meilisearch のデータが消失または欠損した場合は、MariaDB から再同期するための仕組みが必要である。

考えられる再同期手段の一つとして

1. Meilisearch の `/stats` API を使用して作成済みの index 数を取得したうえで、MariaDB 上のデータ数を比較し、一定の閾値を下回った時点で通知を行う
2. 必要があれば手動(自動にしても良いかもしれないが、検討が必要)で差分を使用して同期 or 全件再同期を行う(ただし、Web
   上でボタンを押すことで再同期を実行できることが望ましい。)

がある。これは seichi-portal-backend が受け持つ事できる。
