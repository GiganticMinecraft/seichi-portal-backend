use async_trait::async_trait;
use domain::message::models::Message;
use errors::infra::InfraError;

use crate::database::{
    components::MessageDatabase,
    connection::{execute_and_values, ConnectionPool},
};

#[async_trait]
impl MessageDatabase for ConnectionPool {
    async fn post_message(&self, message: &Message) -> Result<(), InfraError> {
        let id = message.id().to_string().to_owned();
        let related_answer_id = message.related_answer().id.into_inner().to_owned();
        let posted_user = message.posted_user().id.to_string().to_owned();
        let body = message.body().to_owned();
        let timestamp = message.timestamp().to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(r"INSERT INTO messages (id, related_answer_id, posted_user, body, timestamp) VALUES (?, ?, ?, ?, ?)", [
                    id.into(),
                    related_answer_id.into(),
                    posted_user.into(),
                    body.into(),
                    timestamp.into(),
                ], txn).await?;

                Ok::<_, InfraError>(())
            })

        }).await
            .map_err(Into::into)
    }
}
