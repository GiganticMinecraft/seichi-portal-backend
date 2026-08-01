use std::{
    fmt,
    sync::{Arc, OnceLock},
};

use chrono::{SecondsFormat, Utc};
use opentelemetry::trace::TraceContextExt;
use serde_json::Value;
use tracing::{
    Event, Subscriber,
    dispatcher::WeakDispatch,
    field::{Field, Visit},
};
use tracing_opentelemetry::get_otel_context;
use tracing_subscriber::{
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
};

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

/// ログイベントを 1 行の JSON にフォーマットし、OTel の trace_id / span_id を注入する。
///
/// `tracing-subscriber` 標準の JSON フォーマッタは OTel の trace_id を出力できないため
/// 自前実装している。`trace_id` / `span_id` フィールドは Grafana (Tempo の tracesToLogsV2)
/// との契約であり、フィールド名を変える場合は seichi_infra 側の設定も直すこと。
///
/// span のフィールドはスパン属性として Tempo 側に送られるため、ログ行には出力しない
/// (URL クエリなどリクエスト由来の値がログへ漏れるのを防ぐ意図もある)。
///
/// trace_id の解決には subscriber の [`Dispatch`](tracing::Dispatch) が必要だが、
/// フォーマッタ実行中は tracing の再入ガードにより
/// `Span::current()` や `dispatcher::get_default` が使えない。
/// このため subscriber の init 後に [`JsonWithTraceId::connect_dispatch`] を呼び、
/// trace_id 解決に使う `Dispatch` を登録する必要がある。
#[derive(Clone, Default)]
pub struct JsonWithTraceId {
    dispatch: Arc<OnceLock<WeakDispatch>>,
}

impl JsonWithTraceId {
    pub fn new() -> Self {
        Self::default()
    }

    /// subscriber の init 後 (またはテストでは `with_default` スコープ内) に呼び、
    /// trace_id 解決に使う `Dispatch` を登録します。
    /// 呼ばれないままの場合、ログは出力されるが trace_id / span_id が付かない。
    pub fn connect_dispatch(&self) {
        tracing::dispatcher::get_default(|dispatch| {
            let _ = self.dispatch.set(dispatch.downgrade());
        });
    }
}

impl<S, N> FormatEvent<S, N> for JsonWithTraceId
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut collector = FieldCollector::default();
        event.record(&mut collector);

        let mut entries = vec![
            (
                "timestamp",
                Value::from(Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)),
            ),
            ("level", Value::from(event.metadata().level().as_str())),
            ("target", Value::from(event.metadata().target())),
            (
                "message",
                Value::from(collector.message.unwrap_or_default()),
            ),
        ];
        entries.extend(
            collector
                .fields
                .iter()
                .map(|(name, value)| (*name, value.to_owned())),
        );

        let otel_context = self
            .dispatch
            .get()
            .and_then(WeakDispatch::upgrade)
            .and_then(|dispatch| {
                let event_span = ctx.event_scope()?.next()?;
                get_otel_context(&event_span.id(), &dispatch)
            });
        if let Some(otel_context) = otel_context {
            let span_ref = otel_context.span();
            let span_context = span_ref.span_context();
            if span_context.is_valid() {
                entries.push(("trace_id", Value::from(span_context.trace_id().to_string())));
                entries.push(("span_id", Value::from(span_context.span_id().to_string())));
            }
        }

        write!(writer, "{{")?;
        for (index, (key, value)) in entries.iter().enumerate() {
            if index > 0 {
                write!(writer, ",")?;
            }
            write!(writer, "{}:{value}", Value::from(*key))?;
        }
        writeln!(writer, "}}")
    }
}

#[derive(Default)]
struct FieldCollector {
    message: Option<String>,
    fields: Vec<(&'static str, Value)>,
}

impl FieldCollector {
    fn push(&mut self, field: &Field, value: Value) {
        match (field.name(), value) {
            ("message", Value::String(message)) => self.message = Some(message),
            ("message", value) => self.message = Some(value.to_string()),
            (name, value) => self.fields.push((name, value)),
        }
    }
}

impl Visit for FieldCollector {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push(field, Value::from(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field, Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push(field, Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field, Value::from(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, Value::from(value));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.push(field, Value::from(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push(field, Value::from(format!("{value:?}")));
    }
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

    use super::{JsonWithTraceId, json_logs_enabled};

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
        let formatter = JsonWithTraceId::new();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .event_format(formatter.clone())
                .with_writer(capture.clone()),
        );

        tracing::subscriber::with_default(subscriber, || {
            formatter.connect_dispatch();
            info!(form_id = "0198c6b3", "hello");
        });

        let json = captured_json(&capture);
        assert_eq!(json["message"], "hello");
        assert_eq!(json["level"], "INFO");
        assert_eq!(json["form_id"], "0198c6b3");
        assert!(
            json.get("trace_id").is_none(),
            "OTel の span がなければ trace_id は出力しない"
        );
    }

    #[test]
    fn injects_trace_and_span_id_inside_otel_span() {
        let capture = Capture::default();
        let formatter = JsonWithTraceId::new();
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")))
            .with(
                tracing_subscriber::fmt::layer()
                    .event_format(formatter.clone())
                    .with_writer(capture.clone()),
            );

        tracing::subscriber::with_default(subscriber, || {
            formatter.connect_dispatch();
            let span = tracing::info_span!("request");
            let _guard = span.enter();
            info!("with trace");
        });

        let json = captured_json(&capture);
        let trace_id = json["trace_id"].as_str().expect("trace_id must be present");
        assert_eq!(trace_id.len(), 32);
        assert!(trace_id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(
            trace_id,
            "0".repeat(32),
            "trace_id must be valid (non-zero)"
        );

        let span_id = json["span_id"].as_str().expect("span_id must be present");
        assert_eq!(span_id.len(), 16);
    }
}
