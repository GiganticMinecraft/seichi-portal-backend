//! 検索エンジン同期のメトリクス。

use std::sync::LazyLock;

use opentelemetry::{
    KeyValue, global,
    metrics::{Gauge, Histogram},
};
use resource::messaging::telemetry::CDC_LAG_BUCKETS_SECONDS;

const METER_NAME: &str = "seichi-portal-backend";

/// 同期 1 回 (リトライを含めず 1 試行) の結果。メトリクスの `outcome` 属性になる。
#[derive(Clone, Copy, Debug)]
pub(crate) enum SyncOutcome {
    Success,
    Error,
}

impl SyncOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
        }
    }
}

pub(crate) struct SearchSyncMetrics {
    sync_duration: Histogram<f64>,
    sync_lag: Histogram<f64>,
    out_of_sync_indexes: Gauge<u64>,
}

impl SearchSyncMetrics {
    /// 検索エンジンへの同期 1 試行にかかった時間を記録する。
    pub(crate) fn record_sync_attempt(&self, index: &str, outcome: SyncOutcome, seconds: f64) {
        self.sync_duration.record(
            seconds,
            &[
                KeyValue::new("search.index", index.to_owned()),
                KeyValue::new("outcome", outcome.as_str()),
            ],
        );
    }

    /// MariaDB でコミットされてから検索エンジンに反映し終わるまでの時間を記録する。
    pub(crate) fn record_sync_lag(&self, index: &str, seconds: f64) {
        self.sync_lag
            .record(seconds, &[KeyValue::new("search.index", index.to_owned())]);
    }

    /// 件数が DB と食い違っている検索インデックスの数を記録する。
    pub(crate) fn record_out_of_sync_indexes(&self, count: usize) {
        self.out_of_sync_indexes.record(count as u64, &[]);
    }
}

/// global meter provider の設定より後 (同期タスクの開始後) に初めて参照されるよう遅延初期化する。
pub(crate) static SEARCH_SYNC_METRICS: LazyLock<SearchSyncMetrics> = LazyLock::new(|| {
    let meter = global::meter(METER_NAME);

    SearchSyncMetrics {
        sync_duration: meter
            .f64_histogram("seichi_portal.search.sync.duration")
            .with_description("CDC イベント 1 件を検索エンジンへ同期する 1 試行にかかった時間")
            .with_unit("s")
            .with_boundaries(CDC_LAG_BUCKETS_SECONDS.to_vec())
            .build(),
        sync_lag: meter
            .f64_histogram("seichi_portal.search.sync.lag")
            .with_description("MariaDB でコミットされてから検索エンジンに反映し終わるまでの時間")
            .with_unit("s")
            .with_boundaries(CDC_LAG_BUCKETS_SECONDS.to_vec())
            .build(),
        out_of_sync_indexes: meter
            .u64_gauge("seichi_portal.search.out_of_sync_indexes")
            .with_description("定期チェックで DB と件数が食い違っていた検索インデックスの数")
            .with_unit("{index}")
            .build(),
    }
});
