use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{form::models::PostedAnswers, message::models::Message, user::models::Role};
use errors::infra::InfraError;
use itertools::Itertools;

use crate::{
    database::{
        components::MessageDatabase,
        connection::{execute_and_values, query_all_and_values, ConnectionPool},
    },
    dto::{AnswerDto, CommentDto, LabelDto, MessageDto, PostedAnswersDto, UserDto},
};

#[async_trait]
impl MessageDatabase for ConnectionPool {
    #[tracing::instrument(skip(self))]
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

    #[tracing::instrument(skip(self))]
    async fn fetch_messages_by_answer_id(
        &self,
        answers: &PostedAnswers,
    ) -> Result<Vec<MessageDto>, InfraError> {
        let answer_id = answers.id.into_inner().to_owned();

        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let rs = query_all_and_values(
                        r"SELECT id, posted_user, name, role, body, timestamp FROM messages
                    INNER JOIN ON users.id = messages.posted_user
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
                                    id: uuid::Uuid::from_str(
                                        &rs.try_get::<String>("", "posted_user")?,
                                    )?,
                                    role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                                });

                                Ok::<_, InfraError>((
                                    user?,
                                    uuid::Uuid::from_str(&rs.try_get::<String>("", "id")?)?,
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
                related_answer: PostedAnswersDto {
                    id: answers.id.into_inner().to_owned(),
                    user_name: answers.user.name.to_owned(),
                    uuid: answers.user.id,
                    user_role: answers.user.role.to_owned(),
                    timestamp: answers.timestamp,
                    form_id: answers.form_id.into_inner().to_owned(),
                    title: answers.title.default_answer_title.to_owned(),
                    answers: answers
                        .answers
                        .iter()
                        .map(|answer| AnswerDto {
                            question_id: answer.question_id.into_inner().to_owned(),
                            answer: answer.answer.to_owned(),
                        })
                        .collect_vec(),
                    comments: answers
                        .comments
                        .iter()
                        .map(|comment| CommentDto {
                            answer_id: comment.answer_id.into_inner().to_owned(),
                            comment_id: comment.comment_id.into_inner().to_owned(),
                            content: comment.content.to_owned(),
                            timestamp: comment.timestamp,
                            commented_by: UserDto {
                                name: comment.commented_by.name.to_owned(),
                                id: comment.commented_by.id,
                                role: comment.commented_by.role.to_owned(),
                            },
                        })
                        .collect_vec(),
                    labels: answers
                        .labels
                        .iter()
                        .map(|label| LabelDto {
                            id: label.id.into_inner().to_owned(),
                            name: label.name.to_owned(),
                        })
                        .collect_vec(),
                },
                posted_user: user,
                body,
                timestamp,
            })
            .collect_vec())
    }
}
