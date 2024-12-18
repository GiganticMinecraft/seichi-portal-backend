use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    form::{
        answer::models::FormAnswer,
        message::models::{Message, MessageId},
    },
    user::models::Role,
};
use errors::infra::InfraError;
use itertools::Itertools;

use crate::{
    database::{
        components::FormMessageDatabase,
        connection::{
            execute_and_values, query_all_and_values, query_one_and_values, ConnectionPool,
        },
    },
    dto::{FormAnswerDto, MessageDto, UserDto},
};

#[async_trait]
impl FormMessageDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn post_message(&self, message: &Message) -> Result<(), InfraError> {
        let id = message.id().to_string().to_owned();
        let related_answer_id = message.related_answer().id.into_inner().to_owned();
        let sender = message.sender().id.to_string().to_owned();
        let body = message.body().to_owned();
        let timestamp = message.timestamp().to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(r"INSERT INTO messages (id, related_answer_id, sender, body, timestamp) VALUES (?, ?, ?, ?, ?)", [
                    id.into(),
                    related_answer_id.into(),
                    sender.into(),
                    body.into(),
                    timestamp.into(),
                ], txn).await?;

                Ok::<_, InfraError>(())
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn update_message_body(
        &self,
        message_id: MessageId,
        body: String,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE messages SET body = ? WHERE id = ?",
                    [body.into(), message_id.into_inner().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn fetch_messages_by_form_answer(
        &self,
        answers: &FormAnswer,
    ) -> Result<Vec<MessageDto>, InfraError> {
        let answer_id = answers.id.into_inner().to_owned();

        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let rs = query_all_and_values(
                        r"SELECT messages.id AS message_id, sender, name, role, body, timestamp FROM messages
                    INNER JOIN users ON users.id = messages.sender
                    WHERE related_answer_id = ?",
                        [answer_id.into()],
                        txn,
                    )
                        .await?;

                    Ok::<_, InfraError>(
                        rs.into_iter()
                            .map(|rs| {
                                let user = Ok::<_, InfraError>(UserDto {
                                    name: rs.try_get("", "name")?,
                                    id: uuid::Uuid::from_str(&rs.try_get::<String>("", "sender")?)?,
                                    role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                                });

                                Ok::<_, InfraError>((
                                    user?,
                                    uuid::Uuid::from_str(&rs.try_get::<String>("", "message_id")?)?,
                                    rs.try_get::<String>("", "body")?,
                                    rs.try_get::<DateTime<Utc>>("", "timestamp")?,
                                ))
                            })
                            .collect_vec(),
                    )
                })
            })
            .await?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(user, message_id, body, timestamp)| MessageDto {
                id: message_id,
                related_answer: FormAnswerDto {
                    id: answers.id.into_inner().to_owned(),
                    user_name: answers.user.name.to_owned(),
                    uuid: answers.user.id,
                    user_role: answers.user.role.to_owned(),
                    timestamp: answers.timestamp,
                    form_id: answers.form_id.into_inner().to_owned(),
                    title: answers.title.to_owned(),
                },
                sender: user,
                body,
                timestamp,
            })
            .collect_vec())
    }

    #[tracing::instrument]
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<Option<MessageDto>, InfraError> {
        let message_id = message_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rs = query_one_and_values(
                    r"SELECT sender, message_senders.name, message_senders.role, body, timestamp,
                    answers.id AS answer_id,
                    time_stamp,
                    form_id,
                    user AS respondent_id,
                    respondents.name AS respondent_name,
                    respondents.role AS respondent_role
                    FROM messages
                    INNER JOIN answers ON related_answer_id = answers.id
                    INNER JOIN users AS message_senders ON message_senders.id = messages.sender
                    INNER JOIN users AS respondents ON respondents.id = answers.user
                    WHERE messages.id = ?",
                    [message_id.to_string().into()],
                    txn,
                )
                .await?;

                rs.map(|rs| {
                    let user = Ok::<_, InfraError>(UserDto {
                        name: rs.try_get("", "name")?,
                        id: uuid::Uuid::from_str(&rs.try_get::<String>("", "sender")?)?,
                        role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                    })?;

                    let related_answer = Ok::<_, InfraError>(FormAnswerDto {
                        id: rs.try_get("", "answer_id")?,
                        user_name: rs.try_get("", "respondent_name")?,
                        uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "respondent_id")?)?,
                        user_role: Role::from_str(&rs.try_get::<String>("", "respondent_role")?)?,
                        timestamp: rs.try_get("", "time_stamp")?,
                        form_id: rs.try_get("", "form_id")?,
                        title: rs.try_get("", "title")?,
                    })?;

                    Ok::<_, InfraError>(MessageDto {
                        id: message_id.to_owned(),
                        related_answer,
                        sender: user,
                        body: rs.try_get("", "body")?,
                        timestamp: rs.try_get("", "timestamp")?,
                    })
                })
                .transpose()
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn delete_message(&self, message_id: MessageId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM messages WHERE id = ?",
                    [message_id.to_string().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }
}
