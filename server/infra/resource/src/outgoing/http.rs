use std::{sync::LazyLock, time::Duration};

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;

/// 外向き HTTP の TCP / TLS 接続確立の上限。
///
/// 宛先がブラックホール (SYN に応答が無い) のときに OS の接続タイムアウト (数十秒〜数分) まで
/// 待たないようにする。クラスタ内 (Meilisearch) / Discord / Mojang / Cloudflare のどれも
/// 通常は数十 ms で確立するため、3 秒あれば十分。
pub const OUTGOING_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// 外向き HTTP の 1 リクエスト全体 (接続〜レスポンスボディ受信完了) の上限。
///
/// これより短くしたい呼び出し (Turnstile など) は `RequestBuilder::timeout` で個別に上書きする。
pub const OUTGOING_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// アプリ全体で共有する HTTP クライアント。
///
/// reqwest の `Client` は内部にコネクションプールを持つため、
/// コールサイトごとに生成せずこれを使い回す。
///
/// 既定で [`OUTGOING_HTTP_CONNECT_TIMEOUT`] / [`OUTGOING_HTTP_REQUEST_TIMEOUT`] が掛かる。
/// 上限が無いと宛先が応答しないときにリクエストを処理するタスクが無期限に残り、
/// 呼び出し元の API リクエストや graceful shutdown が詰まる。
///
/// [`TracingMiddleware`] がリクエストごとにクライアントスパン
/// (OTel semantic conventions 準拠の HTTP 属性付き) を作り、
/// 現在のスパンの trace context (W3C `traceparent`) を outbound リクエストへ注入する。
/// OTel が初期化されていない場合は global propagator が no-op のため何も注入されない。
pub static HTTP_CLIENT: LazyLock<ClientWithMiddleware> = LazyLock::new(|| {
    let client = reqwest::Client::builder()
        .connect_timeout(OUTGOING_HTTP_CONNECT_TIMEOUT)
        .timeout(OUTGOING_HTTP_REQUEST_TIMEOUT)
        .build()
        .expect("failed to build the shared outgoing HTTP client");

    ClientBuilder::new(client)
        .with(TracingMiddleware::default())
        .build()
});
