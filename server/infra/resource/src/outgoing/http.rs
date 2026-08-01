use std::sync::LazyLock;

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;

/// アプリ全体で共有する HTTP クライアント。
///
/// reqwest の `Client` は内部にコネクションプールを持つため、
/// コールサイトごとに生成せずこれを使い回す。
///
/// [`TracingMiddleware`] がリクエストごとにクライアントスパン
/// (OTel semantic conventions 準拠の HTTP 属性付き) を作り、
/// 現在のスパンの trace context (W3C `traceparent`) を outbound リクエストへ注入する。
/// OTel が初期化されていない場合は global propagator が no-op のため何も注入されない。
pub static HTTP_CLIENT: LazyLock<ClientWithMiddleware> = LazyLock::new(|| {
    ClientBuilder::new(reqwest::Client::new())
        .with(TracingMiddleware::default())
        .build()
});
