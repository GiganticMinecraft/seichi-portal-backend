//! グローバルな subscriber を設定するため、ほかのテストと別プロセスになる統合テストに分けている。

use std::{
    io,
    sync::{Arc, Mutex},
};

use entrypoint::{logging::stdout_log_layer_with_writer, telemetry::otel_span_filter};
use opentelemetry::trace::TracerProvider as _;
use tracing_subscriber::{
    Layer as _, fmt::MakeWriter, layer::SubscriberExt as _, util::SubscriberInitExt as _,
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

/// sqlx の QueryLogger はクエリごとに `log::log_enabled!` と `tracing::enabled!` で問い合わせる。どのレイヤーも無効と
/// 答えると per-layer filter の判定結果がスレッドローカルに残り (tracing-subscriber 0.3.23)、
/// 次のイベントのコールサイトが `Interest::always` だと判定がやり直されないため、残った結果で
/// アクセスログ (と OTel のスパンイベント) が捨てられていた。本番と同じ組み立て・グローバルな
/// subscriber で、問い合わせの後のイベントが捨てられないこと
#[test]
fn event_after_globally_disabled_enabled_check_is_not_dropped() {
    let capture = Capture::default();
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
    let subscriber = tracing_subscriber::registry()
        .with(Some(
            tracing_opentelemetry::layer()
                .with_tracer(provider.tracer("test"))
                .with_filter(otel_span_filter()),
        ))
        .with(stdout_log_layer_with_writer(true, None, capture.clone()));
    // 本番 (main.rs) と同じく init() で設定する。log クレートのイベントを tracing へ流す
    // LogTracer もここで入る
    subscriber.init();

    let request = tracing::info_span!(target: "otel::tracing", "GET /api/v1/forms");
    let _request = request.enter();
    for _ in 0..3 {
        // sqlx の QueryLogger と同じ問い合わせ (イベント自体は出さない)。log 側は LogTracer 経由で
        // コールサイトのキャッシュなしに毎回 enabled が呼ばれる
        let _ = tracing::log::log_enabled!(target: "sqlx::query", tracing::log::Level::Debug);
        let _ = tracing::enabled!(target: "sqlx::query", tracing::Level::DEBUG);
        tracing::info!(target: "entrypoint::access", "request completed");
    }

    let bytes = capture.0.lock().unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(
        text.matches("request completed").count(),
        3,
        "output: {text:?}"
    );
}
