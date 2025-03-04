use crate::messaging::config::{RABBITMQ, RabbitMQ};
use errors::infra::InfraError;
use futures::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, QueueDeclareOptions};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};
use std::sync::Arc;
use tokio::sync::Notify;

pub struct ConnectionPool {
    pub(crate) rabbitmq_client: Connection,
    shutdown_notify: Arc<Notify>,
}

impl ConnectionPool {
    pub async fn new() -> Self {
        let RabbitMQ {
            user,
            password,
            host,
            port,
            ..
        } = &*RABBITMQ;

        let addr = format!("amqp://{user}:{password}@{host}:{port}/%2f");

        let connection = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .expect("Cannot establish connect to RabbitMQ.");

        Self {
            rabbitmq_client: connection,
            shutdown_notify: Arc::new(Notify::new()),
        }
    }

    async fn create_channel(&self) -> Result<Channel, InfraError> {
        Ok(self.rabbitmq_client.create_channel().await?)
    }

    pub async fn consumer(&self) -> Result<(), InfraError> {
        let RabbitMQ { routing_key, .. } = &*RABBITMQ;

        let channel = self.create_channel().await?;

        channel
            .queue_declare(
                routing_key,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await?;

        let mut consumer = channel
            .basic_consume(
                routing_key,
                "",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        tokio::spawn({
            let shutdown_notify = self.shutdown_notify.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = shutdown_notify.notified() => {
                            break;
                        },
                        _ = async {
                            if let Some(Ok(delivery)) = consumer.next().await {
                                let data = String::from_utf8_lossy(&delivery.data);
                                println!("Received message: {:?}", data);

                                delivery.ack(BasicAckOptions::default()).await?;
                            }
                            Ok::<_, InfraError>(())
                        } => {}
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn shutdown(&self) {
        tracing::info!("Shutting down messaging connection...");

        self.shutdown_notify.notify_waiters()
    }
}
