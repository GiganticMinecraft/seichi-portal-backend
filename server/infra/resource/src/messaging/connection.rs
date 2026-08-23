use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use domain::search::models::{SearchableFields, SearchableFieldsWithOperation};
use errors::infra::InfraError;
use futures::StreamExt;
use lapin::{
    Connection, ConnectionProperties,
    options::{BasicAckOptions, BasicConsumeOptions, QueueDeclareOptions},
    types::FieldTable,
};
use tokio::sync::{Notify, mpsc};
use tracing::Instrument;

use crate::messaging::{
    config::{RABBITMQ, RabbitMQ},
    schema::{Operation, RabbitMQSchema},
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
    sender: mpsc::Sender<SearchableFieldsWithOperation>,
    rabbitmq_connected: AtomicBool,
}

impl MessagingConnectionPool {
    pub fn new(sender: mpsc::Sender<SearchableFieldsWithOperation>) -> Self {
        Self {
            shutdown_notify: Arc::new(Notify::new()),
            sender,
            rabbitmq_connected: AtomicBool::new(false),
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
                    Some(Ok(delivery)) => {
                        // Debezium CDC 由来のメッセージには trace context がないため、
                        // delivery ごとに新しいルートスパンを作る
                        let span = tracing::info_span!(
                            parent: None,
                            "cdc.process",
                            otel.kind = "consumer",
                            messaging.system = "rabbitmq",
                            messaging.operation.type = "process",
                            messaging.destination.name = %RABBITMQ.routing_key,
                        );
                        async {
                            let data = String::from_utf8_lossy(&delivery.data);
                            let payload = serde_json::from_str::<RabbitMQSchema>(&data)?.payload;

                            let operation = match payload.op.to_owned() {
                                Operation::Create => domain::search::models::Operation::Create,
                                Operation::Update => domain::search::models::Operation::Update,
                                Operation::Delete => domain::search::models::Operation::Delete,
                            };
                            let data_fields = match operation {
                                domain::search::models::Operation::Create | domain::search::models::Operation::Update => {
                                    payload.try_into_after()?
                                }
                                domain::search::models::Operation::Delete => {
                                    payload.try_into_before()?
                                }
                            };

                            if let Some(data_fields) = data_fields {
                                self.sender
                                    .send((SearchableFields::try_from(data_fields)?, operation))
                                    .await?;
                            }

                            delivery.ack(BasicAckOptions::default()).await?;
                            Ok::<_, InfraError>(())
                        }
                        .instrument(span)
                        .await
                    }
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

    pub async fn shutdown(&self) {
        tracing::info!("Shutting down messaging connection...");

        self.rabbitmq_connected.store(false, Ordering::Release);
        self.shutdown_notify.notify_one()
    }
}
