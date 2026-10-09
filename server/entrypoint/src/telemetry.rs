use opentelemetry::{KeyValue, global, metrics::ObservableGauge};
use opentelemetry_otlp::{MetricExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource, metrics::SdkMeterProvider, propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use tokio::runtime::Handle;

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
    use super::init_providers;

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
}
