//! トレースの起点の種類 (`seichi_portal.flow` 属性)。
//!
//! 利用者の操作を起点とする同期的な流れと、イベントを起点とする非同期の流れでは
//! 見たいものが違うため、各流れの最初の backend スパンにこの属性を付け、
//! Tempo で `{ span.seichi_portal.flow = "cdc" }` のように絞り込めるようにする。

/// スパン属性のキー
pub const ATTRIBUTE: &str = "seichi_portal.flow";

/// 利用者の操作を起点とする HTTP リクエスト (画面表示・画面操作・ログイン)
pub const USER: &str = "user";
/// MariaDB の変更を Debezium → RabbitMQ 経由で受け取り、検索エンジンへ反映する流れ
pub const CDC: &str = "cdc";
/// 利用者の操作を契機とするアプリケーションイベントから、外部 (Discord) へ通知する流れ
pub const NOTIFICATION: &str = "notification";
/// 定期実行 (検索エンジンとの件数乖離チェックなど)
pub const SCHEDULED: &str = "scheduled";
