use std::sync::Arc;

use domain::search::models::SearchableFields;
use errors::infra::InfraError;
use futures::StreamExt;
use lapin::{
    Channel, Connection, ConnectionProperties,
    options::{BasicAckOptions, BasicConsumeOptions, QueueDeclareOptions},
    types::FieldTable,
};
use tokio::sync::{Notify, mpsc};

use crate::messaging::{
    config::{RABBITMQ, RabbitMQ},
    schema::RabbitMQSchema,
};

pub struct MessagingConnectionPool {
    pub(crate) rabbitmq_client: Connection,
    shutdown_notify: Arc<Notify>,
    sender: mpsc::Sender<SearchableFields>,
}

impl MessagingConnectionPool {
    pub async fn new(sender: mpsc::Sender<SearchableFields>) -> Self {
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
            sender,
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
            let sender = self.sender.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = shutdown_notify.notified() => {
                            break;
                        },
                        _ = async {
                            if let Some(Ok(delivery)) = consumer.next().await {
                                let data = String::from_utf8_lossy(&delivery.data);
                                let payload = serde_json::from_str::<RabbitMQSchema>(&data)?.payload;
                                let after = payload.try_into_after()?;

                                if let Some(after) = after {
                                    sender.send(SearchableFields::try_from(after)?).await?;
                                }

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
