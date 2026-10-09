use opentelemetry::{KeyValue, global, metrics::ObservableGauge};
use opentelemetry_otlp::{MetricExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource, metrics::SdkMeterProvider, propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use common::trace_flow;
use tokio::runtime::Handle;
use tracing::{Level, Metadata, Span, subscriber::Interest};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{
    filter::{LevelFilter, Targets},
    layer::{Context, Filter},
};

const SERVICE_NAME: &str = "seichi-portal-backend";

/// OpenTelemetry のトレースとメトリクスの provider。
///
/// shutdown 時に残りのデータを flush するため、プロセス終了まで保持する。
pub struct TelemetryProviders {
    pub tracer_provider: SdkTracerProvider,
    pub meter_provider: SdkMeterProvider,
}

impl TelemetryProviders {
    /// 残りのスパンとメトリクスを blocking export して provider を止める。
    /// 失敗した provider ごとのエラーメッセージを返す。
    pub fn shutdown(self) -> Vec<String> {
        [
            self.tracer_provider
                .shutdown()
                .err()
                .map(|error| format!("tracer provider: {error}")),
            self.meter_provider
                .shutdown()
                .err()
                .map(|error| format!("meter provider: {error}")),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// OpenTelemetry のトレースとメトリクスを初期化します。
///
/// `OTEL_SDK_DISABLED=true` または `OTEL_EXPORTER_OTLP_ENDPOINT` 未設定の場合は
/// 初期化をスキップして `None` を返します
/// (`OTEL_SDK_DISABLED` は Rust SDK 未実装のため自前でゲートしています)。
///
/// エクスポートは OTLP http/protobuf で、endpoint やメトリクスの送信間隔
/// (`OTEL_METRIC_EXPORT_INTERVAL`) などの設定は `OTEL_*` 環境変数から自動で読み込まれます。
/// メトリクスは Prometheus へ流すため、SDK 既定の cumulative temporality のまま送ります。
pub fn init_providers() -> Option<TelemetryProviders> {
    let sdk_disabled =
        std::env::var("OTEL_SDK_DISABLED").is_ok_and(|value| value.eq_ignore_ascii_case("true"));
    let endpoint_configured =
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok_and(|value| !value.is_empty());

    if sdk_disabled || !endpoint_configured {
        return None;
    }

    global::set_text_map_propagator(TraceContextPropagator::new());

    let resource = resource();

    let span_exporter = SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()
        .expect("failed to build OTLP span exporter");
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(resource.clone())
        .build();
    global::set_tracer_provider(tracer_provider.clone());

    let metric_exporter = MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()
        .expect("failed to build OTLP metric exporter");
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(metric_exporter)
        .with_resource(resource)
        .build();
    global::set_meter_provider(meter_provider.clone());

    Some(TelemetryProviders {
        tracer_provider,
        meter_provider,
    })
}

fn resource() -> Resource {
    // Resource::builder() は OTEL_SERVICE_NAME / OTEL_RESOURCE_ATTRIBUTES を
    // 自動で読むため、service.name は環境変数未設定時のみデフォルト値を与える
    let builder = if std::env::var("OTEL_SERVICE_NAME").is_ok() {
        Resource::builder()
    } else {
        Resource::builder().with_service_name(SERVICE_NAME)
    };

    // service.version はマニフェストに書くとイメージ更新に追従できず乖離するため、
    // ビルド時のバージョンを埋め込む
    builder
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build()
}

/// OpenTelemetry へ送るスパンを、トレースとして意味のあるものに絞るフィルタ。
///
/// 許可リスト方式で、次だけを Tempo へ送る。
/// - HTTP サーバースパン (axum-tracing-opentelemetry。target `otel::tracing`)
/// - 外部 HTTP 呼び出し (reqwest-tracing)
/// - このワークスペースのクレートが作るスパン
///
/// lapin / serenity / h2 / hyper などライブラリ内部のスパン (`Connection`、`io_loop`、
/// `Prioritize::queue_frame` など) は、親を持たないルートスパンとして大量に出てトレース検索を
/// 埋めるため送らない。
///
/// `resource::repository` は database 層へ委譲するだけのものが多く、同名スパンが二重に
/// ネストする (`list` → `list` → `mariadb`) ため送らない。除外したスパンの子は、
/// 送られる直近の祖先 (ハンドラーやユースケース) にぶら下がる。
pub fn otel_span_filter() -> Targets {
    Targets::new()
        .with_target("otel::tracing", Level::TRACE)
        .with_target("reqwest_tracing", Level::TRACE)
        .with_target("entrypoint", Level::INFO)
        .with_target("presentation", Level::INFO)
        .with_target("usecase", Level::INFO)
        .with_target("domain", Level::INFO)
        .with_target("resource", Level::INFO)
        .with_target("resource::repository", LevelFilter::OFF)
}

/// stdout へのログ出力レイヤーに掛けるフィルター ([`log_span_filter`] で作る)。
///
/// - [`otel_span_filter`] で Tempo へ送らないスパン (repository 層など) をログ側からも見えなくする。
///   json-subscriber はイベントの直近の親スパンからしか trace_id を取らないため、送らないスパンの
///   中で出たログは trace_id を失う。per-layer filter で隠したスパンは飛ばされ、直近の祖先
///   (= 送られるスパン) が親として扱われるので、ログには常に Tempo にあるスパンの ID が付く。
///   イベント自体は通常どおり EnvFilter の判断に従う。
/// - コールサイトの判定を常に `Interest::sometimes` にし、イベントごとに per-layer filter の判定を
///   やり直させる。tracing-subscriber 0.3.23 では、`log::log_enabled!` (LogTracer 経由) などで
///   どのレイヤーも無効と答えた問い合わせの判定結果がスレッドローカルに残り、次のイベントの
///   コールサイトが `Interest::always` だと判定がやり直されずに、その残りで捨てられてしまう
///   (sqlx がクエリごとに問い合わせるため、DB を使ったリクエストのアクセスログが消えていた)。
pub struct LogSpanFilter {
    exported: Targets,
}

pub fn log_span_filter() -> LogSpanFilter {
    LogSpanFilter {
        exported: otel_span_filter(),
    }
}

impl<S> Filter<S> for LogSpanFilter {
    fn enabled(&self, metadata: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        !metadata.is_span()
            || self
                .exported
                .would_enable(metadata.target(), metadata.level())
    }

    fn callsite_enabled(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }
}

/// HTTP リクエストごとに、トレースへ流れの種類を付け、trace ID 付きのアクセスログを 1 行出す。
///
/// `OtelAxumLayer` の内側に置くこと。そうすると現在のスパンが HTTP サーバースパンになり、
/// JSON ログに `openTelemetry.traceId` が付いて Loki → Tempo の相互リンクに使える。
/// probe (`/health`) はトレースもログも出さない。
pub async fn record_request(request: Request, next: Next) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |path| path.as_str().to_owned());
    if route.starts_with("/health") {
        return next.run(request).await;
    }

    Span::current().set_attribute(trace_flow::ATTRIBUTE, trace_flow::USER);
    let method = request.method().clone();
    let started_at = Instant::now();

    let response = next.run(request).await;
    let status_code = response.status().as_u16();
    let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;

    tracing::info!(
        target: "entrypoint::access",
        method = %method,
        route = %route,
        status = status_code,
        duration_ms,
        "request completed",
    );
    response
}

/// tokio ランタイムの飽和を見るためのメトリクスを登録します。
///
/// `tokio_unstable` を要求しない stable API の値だけを使います。
/// 返り値を drop するとコールバックが外れうるため、プロセス終了まで保持してください。
pub fn register_runtime_metrics(handle: Handle) -> Vec<ObservableGauge<u64>> {
    let meter = global::meter(SERVICE_NAME);
    let alive_tasks_handle = handle.clone();
    let queue_depth_handle = handle.clone();

    vec![
        meter
            .u64_observable_gauge("seichi_portal.runtime.alive_tasks")
            .with_description("tokio ランタイム上で生存しているタスク数")
            .with_unit("{task}")
            .with_callback(move |observer| {
                observer.observe(alive_tasks_handle.metrics().num_alive_tasks() as u64, &[]);
            })
            .build(),
        meter
            .u64_observable_gauge("seichi_portal.runtime.global_queue_depth")
            .with_description("tokio ランタイムのグローバルキューで実行を待っているタスク数")
            .with_unit("{task}")
            .with_callback(move |observer| {
                observer.observe(
                    queue_depth_handle.metrics().global_queue_depth() as u64,
                    &[],
                );
            })
            .build(),
        meter
            .u64_observable_gauge("seichi_portal.runtime.workers")
            .with_description("tokio ランタイムのワーカースレッド数")
            .with_unit("{thread}")
            .with_callback(move |observer| {
                observer.observe(handle.metrics().num_workers() as u64, &[]);
            })
            .build(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{init_providers, otel_span_filter};
    use tracing::Level;

    #[test]
    fn otel_span_filter_keeps_application_spans_and_drops_library_internals() {
        let filter = otel_span_filter();
        let enabled = |target: &str| filter.would_enable(target, &Level::INFO);

        assert!(enabled("otel::tracing"), "HTTP サーバースパン");
        assert!(
            enabled("reqwest_tracing::reqwest_otel_span_builder"),
            "外部 HTTP 呼び出し"
        );
        assert!(enabled("presentation::api::global_discord_webhook"));
        assert!(enabled("usecase::search"));
        assert!(enabled("resource::messaging::connection"));
        assert!(enabled("resource::database::forms::form"));

        assert!(
            !enabled("resource::repository::form_repository_impls::form_repository_impl"),
            "database 層と二重になるため送らない"
        );
        assert!(!enabled("lapin::channel"));
        assert!(!enabled("serenity::gateway::shard"));
        assert!(!enabled("h2::proto::streams::prioritize"));
        assert!(!enabled("hyper::client"));
        assert!(!enabled("sqlx::query"));
    }

    /// 環境変数の設定はプロセス全体に影響するため、
    /// 競合しないよう 1 つのテストで順に検証する。
    #[test]
    fn providers_are_gated_by_environment_variables() {
        // SAFETY: このテストバイナリ内で環境変数を読み書きするのはこのテストだけ
        unsafe {
            std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
            std::env::remove_var("OTEL_SDK_DISABLED");
        }
        assert!(
            init_providers().is_none(),
            "OTEL_EXPORTER_OTLP_ENDPOINT 未設定なら初期化をスキップする"
        );

        unsafe {
            std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4318");
            std::env::set_var("OTEL_SDK_DISABLED", "true");
        }
        assert!(
            init_providers().is_none(),
            "OTEL_SDK_DISABLED=true なら endpoint が設定されていてもスキップする"
        );

        unsafe {
            std::env::remove_var("OTEL_SDK_DISABLED");
        }
        let providers = init_providers();
        assert!(
            providers.is_some(),
            "endpoint 設定時は tracer / meter provider が初期化される"
        );

        // 送信先が無いため最終 export の失敗は許容し、shutdown が返ってくることだけを確認する
        if let Some(providers) = providers {
            let _ = providers.shutdown();
        }
        unsafe {
            std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
        }
    }

    /// HTTP リクエストを 1 本流し、OTel のスパンと stdout のログを受け取る
    mod record_request_middleware {
        use std::{
            io,
            sync::{Arc, Mutex},
        };

        use axum::{Router, middleware, routing::get};
        use axum_tracing_opentelemetry::middleware::OtelAxumLayer;
        use common::trace_flow;
        use opentelemetry::trace::TracerProvider as _;
        use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        use tracing_subscriber::{Layer as _, fmt::MakeWriter, layer::SubscriberExt as _};

        use crate::{
            logging::stdout_log_layer_with_writer,
            telemetry::{otel_span_filter, record_request},
        };

        #[derive(Clone, Default)]
        struct Capture(Arc<Mutex<Vec<u8>>>);

        impl io::Write for Capture {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        impl<'a> MakeWriter<'a> for Capture {
            type Writer = Capture;

            fn make_writer(&'a self) -> Capture {
                self.clone()
            }
        }

        async fn request(path: &str) -> (Vec<SpanData>, Vec<serde_json::Value>) {
            let exporter = InMemorySpanExporter::default();
            let provider = SdkTracerProvider::builder()
                .with_simple_exporter(exporter.clone())
                .build();
            let logs = Capture::default();
            let subscriber = tracing_subscriber::registry()
                .with(Some(
                    tracing_opentelemetry::layer()
                        .with_tracer(provider.tracer("test"))
                        .with_filter(otel_span_filter()),
                ))
                .with(stdout_log_layer_with_writer(true, None, logs.clone()));
            let _guard = tracing::subscriber::set_default(subscriber);

            // main.rs と同じく record_request を OtelAxumLayer の内側に置く
            let app = Router::new()
                .route("/api/v1/forms/{id}", get(|| async { "ok" }))
                .route("/health", get(|| async { "ok" }))
                .layer(middleware::from_fn(record_request))
                .layer(OtelAxumLayer::default());
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

            let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            stream
                .write_all(
                    format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
                        .as_bytes(),
                )
                .await
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).await.unwrap();
            assert!(
                response.starts_with(b"HTTP/1.1 200"),
                "request must succeed"
            );
            server.abort();

            provider.force_flush().expect("flush must succeed");
            let spans = exporter
                .get_finished_spans()
                .expect("spans must be exported");
            let logs = String::from_utf8(logs.0.lock().unwrap().clone())
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).expect("log line must be JSON"))
                .filter(|line: &serde_json::Value| line["target"] == "entrypoint::access")
                .collect();
            (spans, logs)
        }

        #[tokio::test]
        async fn marks_request_as_user_flow_and_logs_with_its_trace_id() {
            let (spans, logs) = request("/api/v1/forms/0198c6b3").await;

            let server = spans
                .iter()
                .find(|span| span.name == "GET /api/v1/forms/{id}")
                .expect("HTTP サーバースパンが送られる");
            let flow = server
                .attributes
                .iter()
                .find(|kv| kv.key.as_str() == trace_flow::ATTRIBUTE)
                .map(|kv| kv.value.to_string());
            assert_eq!(flow.as_deref(), Some(trace_flow::USER));

            assert_eq!(logs.len(), 1, "リクエストごとにアクセスログを 1 行出す");
            let log = &logs[0];
            assert_eq!(
                log["route"], "/api/v1/forms/{id}",
                "パスのパラメーターは出さない"
            );
            assert_eq!(log["method"], "GET");
            assert_eq!(log["status"], 200);
            assert!(log["duration_ms"].is_number());
            assert_eq!(
                log["openTelemetry"]["traceId"],
                server.span_context.trace_id().to_string(),
                "ログから Tempo のトレースへ辿れる"
            );
        }

        #[tokio::test]
        async fn does_not_log_health_checks() {
            let (_, logs) = request("/health").await;
            assert!(logs.is_empty());
        }
    }
}
