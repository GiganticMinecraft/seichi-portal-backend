use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use chrono::Utc;
use domain::search::models::SearchSyncEvent;
use errors::infra::InfraError;
use futures::StreamExt;
use lapin::{
    Connection, ConnectionProperties,
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, QueueDeclareOptions},
    types::FieldTable,
};
use opentelemetry::{global, metrics::ObservableGauge};
use tokio::sync::{Notify, mpsc};
use tracing::{Instrument, Span, field};

use crate::messaging::{
    config::{RABBITMQ, RabbitMQ},
    schema::RabbitMQSchema,
    telemetry::{
        CDC_METRICS, DeliveryOutcome, set_parent_from_amqp_headers, trace_context_carrier,
    },
};

const RABBITMQ_RECONNECT_INTERVAL: Duration = Duration::from_secs(10);

struct ConnectionStatusGuard<'a>(&'a AtomicBool);

impl Drop for ConnectionStatusGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub struct MessagingConnectionPool {
    shutdown_notify: Arc<Notify>,
    sender: mpsc::Sender<SearchSyncEvent>,
    rabbitmq_connected: AtomicBool,
    _pending_sync_events_gauge: ObservableGauge<u64>,
}

impl MessagingConnectionPool {
    /// global meter provider を設定した後に呼ぶこと (それより前だと滞留数のメトリクスが no-op になる)。
    pub fn new(sender: mpsc::Sender<SearchSyncEvent>) -> Self {
        // コールバックが Sender を持ち続けるとチャンネルが閉じず同期タスクが終わらなくなるため、
        // WeakSender で参照する
        let weak_sender = sender.downgrade();
        let pending_sync_events_gauge = global::meter("seichi-portal-backend")
            .u64_observable_gauge("seichi_portal.cdc.pending_sync_events")
            .with_description(
                "CDC consumer から検索エンジン同期タスクへ渡され、まだ処理されていないイベント数",
            )
            .with_unit("{event}")
            .with_callback(move |observer| {
                if let Some(sender) = weak_sender.upgrade() {
                    let pending = sender.max_capacity() - sender.capacity();
                    observer.observe(pending as u64, &[]);
                }
            })
            .build();

        Self {
            shutdown_notify: Arc::new(Notify::new()),
            sender,
            rabbitmq_connected: AtomicBool::new(false),
            _pending_sync_events_gauge: pending_sync_events_gauge,
        }
    }

    pub fn is_rabbitmq_connected(&self) -> bool {
        self.rabbitmq_connected.load(Ordering::Acquire)
    }

    pub async fn consumer(&self) -> Result<(), InfraError> {
        loop {
            let result = tokio::select! {
                _ = self.shutdown_notify.notified() => return Ok(()),
                result = self.consume_once() => result,
            };

            match result {
                Ok(()) => return Ok(()),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        retry_interval_seconds = RABBITMQ_RECONNECT_INTERVAL.as_secs(),
                        "RabbitMQ consumer disconnected; retrying"
                    );
                }
            }

            tokio::select! {
                _ = self.shutdown_notify.notified() => return Ok(()),
                _ = tokio::time::sleep(RABBITMQ_RECONNECT_INTERVAL) => {}
            }
        }
    }

    async fn consume_once(&self) -> Result<(), InfraError> {
        let RabbitMQ {
            user,
            password,
            host,
            port,
            routing_key,
        } = &*RABBITMQ;

        let addr = format!("amqp://{user}:{password}@{host}:{port}/%2f");
        let connection = Connection::connect(&addr, ConnectionProperties::default()).await?;
        let channel = connection.create_channel().await?;

        channel
            .queue_declare(
                routing_key.as_str().into(),
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await?;

        let mut consumer = channel
            .basic_consume(
                routing_key.as_str().into(),
                "".into(),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        self.rabbitmq_connected.store(true, Ordering::Release);
        let _connection_status = ConnectionStatusGuard(&self.rabbitmq_connected);

        loop {
            let result = tokio::select! {
                _ = self.shutdown_notify.notified() => return Ok(()),
                delivery = consumer.next() => match delivery {
                    Some(Ok(delivery)) => self.handle_delivery(delivery).await,
                    Some(Err(error)) => Err(error.into()),
                    None => {
                        return Err(InfraError::Unexpected {
                            cause: "RabbitMQ consumer stream ended".to_string(),
                        });
                    }
                }
            };

            match result {
                Ok(()) => {}
                Err(error) if matches!(&error, InfraError::AMQP { .. }) => return Err(error),
                Err(error) => {
                    tracing::error!(%error, "failed to process RabbitMQ delivery");
                }
            }
        }
    }

    /// 1 件の delivery を処理するスパンを作り、その中で [`Self::process_delivery`] を実行する。
    async fn handle_delivery(&self, delivery: Delivery) -> Result<(), InfraError> {
        let span = tracing::info_span!(
            parent: None,
            "cdc.process",
            otel.kind = "consumer",
            otel.status_code = field::Empty,
            messaging.system = "rabbitmq",
            messaging.operation.type = "process",
            messaging.destination.name = %RABBITMQ.routing_key,
            messaging.message.body.size = delivery.data.len(),
            db.collection.name = field::Empty,
            db.operation.name = field::Empty,
        );
        // Debezium (OpenTelemetry Java Agent) が publish 時に載せた traceparent を親にする
        set_parent_from_amqp_headers(&span, delivery.properties.headers().as_ref());

        let result = self
            .process_delivery(&delivery)
            .instrument(span.clone())
            .await;
        if result.is_err() {
            span.record("otel.status_code", "ERROR");
        }
        result
    }

    async fn process_delivery(&self, delivery: &Delivery) -> Result<(), InfraError> {
        let span = Span::current();
        let data = String::from_utf8_lossy(&delivery.data);
        let payload = match serde_json::from_str::<RabbitMQSchema>(&data) {
            Ok(schema) => schema.payload,
            Err(error) => {
                CDC_METRICS.record_delivery(None, None, DeliveryOutcome::Failed);
                return Err(error.into());
            }
        };

        let table = payload.table().map(ToOwned::to_owned);
        let operation = payload.op.map(|operation| operation.as_str());
        let source_committed_at = payload.source_committed_at();
        if let Some(table) = &table {
            span.record("db.collection.name", table.as_str());
        }
        if let Some(operation) = operation {
            span.record("db.operation.name", operation);
        }
        // ハートビートなどテーブルを持たないイベントは遅延の分布を歪めるため記録しない
        if let (Some(table), Some(committed_at)) = (&table, source_committed_at) {
            let lag = (Utc::now() - committed_at).as_seconds_f64().max(0.0);
            CDC_METRICS.record_receive_lag(table, lag);
        }

        let forwarded = async {
            let Some(fields) = payload.try_into_searchable_fields()? else {
                return Ok(false);
            };
            self.sender
                .send(SearchSyncEvent {
                    fields,
                    source_committed_at,
                    trace_context: trace_context_carrier(&span),
                })
                .await?;
            Ok::<_, InfraError>(true)
        }
        .await;

        let outcome = match &forwarded {
            Ok(true) => DeliveryOutcome::Forwarded,
            Ok(false) => DeliveryOutcome::Skipped,
            Err(_) => DeliveryOutcome::Failed,
        };
        CDC_METRICS.record_delivery(table.as_deref(), operation, outcome);
        forwarded?;

        delivery.ack(BasicAckOptions::default()).await?;
        Ok(())
    }

    pub async fn shutdown(&self) {
        tracing::info!("Shutting down messaging connection...");

        self.rabbitmq_connected.store(false, Ordering::Release);
        self.shutdown_notify.notify_one()
    }
}
