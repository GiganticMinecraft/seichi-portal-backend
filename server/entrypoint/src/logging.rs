use tracing::Subscriber;
use tracing_subscriber::{
    EnvFilter, Layer, filter::FilterExt, fmt::MakeWriter, registry::LookupSpan,
};

use crate::telemetry;

/// stdout ログを JSON にするかどうかを判定します。
///
/// `LOG_FORMAT` 環境変数 (`json` / `pretty`) が設定されていればそれに従い、
/// 未設定ならローカル開発 (`ENV_NAME=local`) でのみ人間向けフォーマットにします。
pub fn json_logs_enabled(env_name: &str, log_format: Option<&str>) -> bool {
    match log_format {
        Some(format) => format.eq_ignore_ascii_case("json"),
        None => env_name != "local",
    }
}

/// stdout へログを出すレイヤーを作ります。
///
/// - `RUST_LOG` (未設定なら `info`) でイベントを絞り、SQL 文 (bind 値を含みうる) は出さない
/// - OTel へ送らないスパンはログ側からも隠す ([`telemetry::log_span_filter`])
///
/// `Option<Layer>` ではなく `Box<dyn Layer>` で返すこと。tracing-subscriber 0.3 の
/// `Option<L>` は `on_register_dispatch` を中の Layer へ渡さないため、json-subscriber が
/// Dispatch を受け取れず、`openTelemetry.traceId` を一切出力しなくなる。
pub fn stdout_log_layer<S>(json: bool, rust_log: Option<&str>) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    stdout_log_layer_with_writer(json, rust_log, std::io::stdout)
}

/// [`stdout_log_layer`] の出力先を差し替えられる版 (テスト用)。
pub fn stdout_log_layer_with_writer<S, W>(
    json: bool,
    rust_log: Option<&str>,
    writer: W,
) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let filter = || {
        EnvFilter::new(rust_log.unwrap_or("info"))
            .add_directive("sqlx::query=off".parse().expect("directive must be valid"))
            .and(telemetry::log_span_filter())
    };

    if json {
        json_log_layer()
            .with_writer(writer)
            .with_filter(filter())
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_filter(filter())
            .boxed()
    }
}

/// stdout ログを 1 行 JSON で出力するレイヤーを作ります。
///
/// `tracing-subscriber` 標準の JSON フォーマッタは OTel の trace_id を出力できないため、
/// [`json_subscriber`] を使う。
///
/// - OTel の span コンテキストが有効な場合、`openTelemetry.traceId` / `openTelemetry.spanId`
///   フィールドが付く (Tempo の tracesToLogsV2 でトレース→ログ相関に使う。
///   このフィールド名は seichi_infra 側の Grafana/Loki 設定との契約であり、
///   変更する場合は両方直すこと)
/// - イベントのフィールドはトップレベルへフラットに出力される
///   (`panic=true` のような LogQL の `| json` パースを前提としたフィールドの契約を保つ)
/// - span のフィールドはスパン属性として Tempo 側へ送られるため、ログ行には出力しない
///   (URL クエリなどリクエスト由来の値がログへ漏れるのを防ぐ意図もある)
pub fn json_log_layer<S>() -> json_subscriber::fmt::Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    json_subscriber::layer()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_opentelemetry_ids(true)
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    use opentelemetry::trace::TracerProvider as _;
    use tracing::info;
    use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt};

    use super::{json_log_layer, json_logs_enabled};

    #[test]
    fn json_logs_are_enabled_outside_local_unless_overridden() {
        assert!(json_logs_enabled("production", None));
        assert!(!json_logs_enabled("local", None));
        assert!(json_logs_enabled("local", Some("json")));
        assert!(!json_logs_enabled("production", Some("pretty")));
    }

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

    fn captured_json(capture: &Capture) -> serde_json::Value {
        let bytes = capture.0.lock().unwrap();
        let text = std::str::from_utf8(&bytes).expect("log output must be valid UTF-8");
        serde_json::from_str(text.lines().next().expect("a log line must be written"))
            .expect("log line must be valid JSON")
    }

    #[test]
    fn formats_event_as_single_line_json() {
        let capture = Capture::default();
        let subscriber =
            tracing_subscriber::registry().with(json_log_layer().with_writer(capture.clone()));

        tracing::subscriber::with_default(subscriber, || {
            info!(form_id = "0198c6b3", "hello");
        });

        let json = captured_json(&capture);
        assert_eq!(json["message"], "hello");
        assert_eq!(json["level"], "INFO");
        assert_eq!(
            json["form_id"], "0198c6b3",
            "イベントフィールドはトップレベルへフラットに出力される"
        );
        assert!(json.get("timestamp").is_some());
        assert!(
            json.get("openTelemetry").is_none(),
            "OTel の span がなければ traceId は出力しない"
        );
    }

    #[test]
    fn injects_trace_and_span_id_inside_otel_span() {
        let capture = Capture::default();
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")))
            .with(json_log_layer().with_writer(capture.clone()));

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("request");
            let _guard = span.enter();
            info!("with trace");
        });

        let json = captured_json(&capture);
        let trace_id = json["openTelemetry"]["traceId"]
            .as_str()
            .expect("traceId must be present");
        assert_eq!(trace_id.len(), 32);
        assert!(trace_id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(trace_id, "0".repeat(32), "traceId must be valid (non-zero)");

        let span_id = json["openTelemetry"]["spanId"]
            .as_str()
            .expect("spanId must be present");
        assert_eq!(span_id.len(), 16);
    }

    /// 本番と同じ組み立て (Option で包んだ OTel レイヤー + Box の stdout レイヤー + グローバルではない
    /// Dispatch 経由の登録) で、OTel へ送らないスパン (repository 層) の中で出たログにも、
    /// 送られる直近の祖先スパンの trace_id が付くこと
    #[test]
    fn log_inside_unexported_span_carries_trace_id_of_exported_ancestor() {
        use tracing_subscriber::{EnvFilter, filter::FilterExt, layer::Layer};

        use crate::telemetry::{log_span_filter, otel_span_filter};

        let capture = Capture::default();
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let stdout_layer = json_log_layer()
            .with_writer(capture.clone())
            .with_filter(EnvFilter::new("info").and(log_span_filter()))
            .boxed();
        let subscriber = tracing_subscriber::registry()
            .with(Some(
                tracing_opentelemetry::layer()
                    .with_tracer(provider.tracer("test"))
                    .with_filter(otel_span_filter()),
            ))
            .with(stdout_layer);

        tracing::subscriber::with_default(subscriber, || {
            // axum-tracing-opentelemetry (tracing_level_info) が作るリクエストのスパン
            let request = tracing::info_span!(target: "otel::tracing", "GET /api/v1/forms");
            let _request = request.enter();
            let repository =
                tracing::info_span!(target: "resource::repository::form_repository_impl", "list");
            let _repository = repository.enter();
            tracing::error!(target: "resource::repository::form_repository_impl", "boom");
        });

        let json = captured_json(&capture);
        assert_eq!(json["message"], "boom", "イベント自体は出力される");
        let trace_id = json["openTelemetry"]["traceId"]
            .as_str()
            .expect("送らないスパンの中でも traceId が付く");
        assert_ne!(trace_id, "0".repeat(32));
    }

    /// `Option` で包むと json-subscriber に Dispatch が渡らず trace_id が消える
    /// (tracing-subscriber 0.3 の挙動)。stdout_log_layer が Box を返す理由の回帰テスト
    #[test]
    fn option_wrapped_json_layer_loses_trace_id() {
        let capture = Capture::default();
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")))
            .with(Some(json_log_layer().with_writer(capture.clone())));

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("request");
            let _guard = span.enter();
            info!("with trace");
        });

        assert!(
            captured_json(&capture).get("openTelemetry").is_none(),
            "この前提が変わったら stdout_log_layer の Box をやめてよい"
        );
    }
}
