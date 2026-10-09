//! CDC consumer のメトリクスと trace context の伝播。
//!
//! Debezium Server の OpenTelemetry Java Agent が RabbitMQ への publish 時に
//! AMQP ヘッダへ `traceparent` を載せるため、受信側はそれを親にして
//! MariaDB の変更 → Debezium → RabbitMQ → 検索エンジン反映を 1 本のトレースにつなぐ。

use std::{collections::HashMap, sync::LazyLock};

use lapin::types::{AMQPValue, FieldTable};
use opentelemetry::{
    Context, KeyValue, global,
    metrics::{Counter, Histogram},
    propagation::Extractor,
    trace::TraceContextExt,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

const METER_NAME: &str = "seichi-portal-backend";

/// CDC の遅延 (秒) 用のバケット境界。
/// 平常時は数十 ms〜数百 ms、検索エンジン停止時などは分単位まで伸びるため広めに取る。
pub const CDC_LAG_BUCKETS_SECONDS: [f64; 15] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 300.0, 900.0,
];

/// 受信したメッセージを処理した結果。メトリクスの `outcome` 属性になる。
#[derive(Clone, Copy, Debug)]
pub enum DeliveryOutcome {
    /// 検索エンジンへの同期タスクへ渡した
    Forwarded,
    /// 検索対象外のテーブル・操作 (ハートビートなど) のため何もせず ack した
    Skipped,
    /// デコードや同期タスクへの受け渡しに失敗した
    Failed,
}

impl DeliveryOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Forwarded => "forwarded",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

pub(crate) struct CdcMetrics {
    deliveries: Counter<u64>,
    receive_lag: Histogram<f64>,
}

impl CdcMetrics {
    /// 1 件の delivery の処理結果を数える。
    ///
    /// `table` / `operation` が取れないメッセージ (デコード失敗やハートビート) は `unknown` にする。
    pub(crate) fn record_delivery(
        &self,
        table: Option<&str>,
        operation: Option<&str>,
        outcome: DeliveryOutcome,
    ) {
        self.deliveries.add(
            1,
            &[
                KeyValue::new("db.collection.name", table.unwrap_or("unknown").to_owned()),
                KeyValue::new(
                    "db.operation.name",
                    operation.unwrap_or("unknown").to_owned(),
                ),
                KeyValue::new("outcome", outcome.as_str()),
            ],
        );
    }

    /// MariaDB でのコミットから backend が受け取るまでの遅延を記録する。
    pub(crate) fn record_receive_lag(&self, table: &str, lag_seconds: f64) {
        self.receive_lag.record(
            lag_seconds,
            &[KeyValue::new("db.collection.name", table.to_owned())],
        );
    }
}

/// 初回利用時に global meter provider から計装を作る。
///
/// global meter provider の設定 (entrypoint の telemetry 初期化) より前に作ると
/// no-op のままになるため、consumer が動き始めてから初めて参照されるよう遅延初期化する。
pub(crate) static CDC_METRICS: LazyLock<CdcMetrics> = LazyLock::new(|| {
    let meter = global::meter(METER_NAME);

    CdcMetrics {
        deliveries: meter
            .u64_counter("seichi_portal.cdc.deliveries")
            .with_description("RabbitMQ から受け取った CDC メッセージの件数")
            .with_unit("{message}")
            .build(),
        receive_lag: meter
            .f64_histogram("seichi_portal.cdc.receive_lag")
            .with_description(
                "MariaDB でコミットされてから backend が CDC メッセージを受け取るまでの時間",
            )
            .with_unit("s")
            .with_boundaries(CDC_LAG_BUCKETS_SECONDS.to_vec())
            .build(),
    }
});

/// AMQP ヘッダ (`FieldTable`) から W3C Trace Context を読むための adapter。
struct AmqpHeaderExtractor<'a>(&'a FieldTable);

impl Extractor for AmqpHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .inner()
            .iter()
            .find(|(name, _)| name.as_str().eq_ignore_ascii_case(key))
            .and_then(|(_, value)| match value {
                AMQPValue::LongString(value) => std::str::from_utf8(value.as_bytes()).ok(),
                AMQPValue::ShortString(value) => Some(value.as_str()),
                _ => None,
            })
    }

    fn keys(&self) -> Vec<&str> {
        self.0.inner().keys().map(|name| name.as_str()).collect()
    }
}

/// AMQP ヘッダに有効な trace context があれば、`span` をその子にする。
///
/// ヘッダが無い (Java Agent 無効の Debezium など) 場合は `span` をルートのまま残す。
pub(crate) fn set_parent_from_amqp_headers(span: &tracing::Span, headers: Option<&FieldTable>) {
    let Some(headers) = headers else {
        return;
    };
    let parent = global::get_text_map_propagator(|propagator| {
        propagator.extract(&AmqpHeaderExtractor(headers))
    });

    if parent.span().span_context().is_valid() {
        // 親の設定に失敗してもトレースが分かれるだけで処理には影響しないため無視する
        let _ = span.set_parent(parent);
    }
}

/// `span` の trace context を、チャンネル越しに渡せるキャリアへ書き出す。
pub(crate) fn trace_context_carrier(span: &tracing::Span) -> HashMap<String, String> {
    let mut carrier = HashMap::new();
    let context: Context = span.context();
    global::get_text_map_propagator(|propagator| propagator.inject_context(&context, &mut carrier));
    carrier
}

#[cfg(test)]
mod tests {
    use super::*;
    use lapin::types::{LongString, ShortString};
    use opentelemetry::propagation::TextMapPropagator;
    use opentelemetry_sdk::propagation::TraceContextPropagator;

    #[test]
    fn extracts_traceparent_from_amqp_long_string_header() {
        let mut headers = FieldTable::default();
        headers.insert(
            ShortString::from("traceparent"),
            AMQPValue::LongString(LongString::from(
                "00-dfd31088b0b5aa721cd09f2393ab84db-fe5c3a1b47f626be-01",
            )),
        );

        let context = TraceContextPropagator::new().extract(&AmqpHeaderExtractor(&headers));
        let span_context = context.span().span_context().clone();

        assert!(
            span_context.is_valid(),
            "traceparent ヘッダから親の span context を復元できる"
        );
        assert_eq!(
            span_context.trace_id().to_string(),
            "dfd31088b0b5aa721cd09f2393ab84db"
        );
    }
}
