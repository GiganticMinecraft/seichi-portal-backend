use std::sync::Arc;

use domain::search::models::{SearchableFields, SearchableFieldsWithOperation};
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
    schema::{Operation, RabbitMQSchema},
};

pub struct MessagingConnectionPool {
    pub(crate) rabbitmq_client: Connection,
    shutdown_notify: Arc<Notify>,
    sender: mpsc::Sender<SearchableFieldsWithOperation>,
}

impl MessagingConnectionPool {
    pub async fn new(sender: mpsc::Sender<SearchableFieldsWithOperation>) -> Self {
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
                                    sender.send((SearchableFields::try_from(data_fields)?, operation)).await?;
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
