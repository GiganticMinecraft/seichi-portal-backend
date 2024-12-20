use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerId, FormAnswerContent},
        models::FormId,
    },
    user::models::{Role, User},
};
use errors::infra::InfraError;

use crate::{
    database::{
        components::FormAnswerDatabase,
        connection::{
            batch_insert, execute_and_values, query_all, query_all_and_values,
            query_one_and_values, ConnectionPool,
        },
    },
    dto::{FormAnswerContentDto, FormAnswerDto},
};

#[async_trait]
impl FormAnswerDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn post_answer(
        &self,
        user: &User,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), InfraError> {
        todo!()
        // let User { id, .. } = user.to_owned();
        // let form_id = form_id.to_owned();
        // let answers = answers.to_owned();
        //
        // self.read_write_transaction(|txn| {
        //     Box::pin(async move {
        //         let regex = Regex::new(r"\$\d+").unwrap();
        //
        //         let default_answer_title_query_result = query_all_and_values(
        //             r"SELECT title FROM default_answer_titles WHERE form_id = ?",
        //             [form_id.to_owned().into_inner().into()],
        //             txn,
        //         )
        //         .await?;
        //
        //         let default_answer_title: Option<String> = default_answer_title_query_result
        //             .first()
        //             .ok_or(FormNotFound {
        //                 id: form_id.to_owned().into_inner(),
        //             })?
        //             .try_get("", "title")?;
        //
        //         // FIXME: ここにドメイン知識が漏れてしまっていることで
        //         //   ここでのドメインエラーが正しくハンドリングできない
        //         let default_answer_title = DefaultAnswerTitle::new(
        //             default_answer_title
        //                 .map(TryInto::try_into)
        //                 .transpose()
        //                 .unwrap(),
        //         )
        //         .to_owned();
        //
        //
        //         let embed_title = regex
        //             .find_iter(default_answer_title.to_string().as_str())
        //             .fold(default_answer_title, |replaced_title, question_id| {
        //                 let answer_opt = answers.iter().find(|answer| {
        //                     answer.question_id.to_string() == question_id.as_str().replace('$', "")
        //                 });
        //                 todo!()
        //                 // replaced_title.into_inner().replace(
        //                 //     question_id.as_str(),
        //                 //     &answer_opt
        //                 //         .map(|answer| answer.answer.to_owned().to_string())
        //                 //         .unwrap_or_default(),
        //                 // )
        //             });
        //
        //         let id = execute_and_values(
        //             r"INSERT INTO answers (form_id, user, title) VALUES (?, ?, ?)",
        //             [
        //                 form_id.to_owned().into_inner().into(),
        //                 id.to_owned().to_string().into(),
        //                 todo!(),
        //                 // embed_title.into(),
        //             ],
        //             txn,
        //         )
        //         .await?
        //         .last_insert_id();
        //
        //         let params = answers
        //             .iter()
        //             .flat_map(|answer| {
        //                 vec![
        //                     id.to_string(),
        //                     answer.question_id.to_string(),
        //                     answer.answer.to_owned(),
        //                 ]
        //             })
        //             .collect_vec();
        //
        //         batch_insert(
        //             "INSERT INTO real_answers (answer_id, question_id, answer) VALUES (?, ?, ?)",
        //             params.into_iter().map(|value| value.into()),
        //             txn,
        //         )
        //         .await?;
        //
        //         Ok::<_, InfraError>(())
        //     })
        // })
        // .await
        // .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answer_query_result_opt = query_one_and_values(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE answers.id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                ).await?;

                answer_query_result_opt
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: answer_id.into_inner(),
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "user")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                        })
                    })
                    .transpose()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContentDto>, InfraError> {
        let answer_id = answer_id.into_inner();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let contents = query_all_and_values(
                    r"SELECT question_id, answer FROM real_answers WHERE answer_id = ?",
                    [answer_id.into()],
                    txn,
                )
                .await?;

                contents
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerContentDto {
                            answer_id,
                            question_id: rs.try_get("", "question_id")?,
                            answer: rs.try_get("", "answer")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = query_all_and_values(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        WHERE form_id = ?
                        ORDER BY answers.time_stamp",
                    [form_id.into_inner().into()],
                    txn,
                ).await?;

                answers
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: rs.try_get("", "answer_id")?,
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "user")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        }).await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_all_answers(&self) -> Result<Vec<FormAnswerDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answers = query_all(
                    r"SELECT form_id, answers.id AS answer_id, title, user, name, role, time_stamp FROM answers
                        INNER JOIN users ON answers.user = users.id
                        ORDER BY answers.time_stamp",
                    txn,
                ).await?;

                answers
                    .iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(FormAnswerDto {
                            id: rs.try_get("", "answer_id")?,
                            uuid: uuid::Uuid::from_str(&rs.try_get::<String>("", "user")?)?,
                            user_name: rs.try_get("", "name")?,
                            user_role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            timestamp: rs.try_get("", "time_stamp")?,
                            form_id: rs.try_get("", "form_id")?,
                            title: rs.try_get("", "title")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), InfraError> {
        let title = title.to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                if let Some(title) = title {
                    execute_and_values(
                        r"UPDATE answers SET title = ? WHERE id = ?",
                        [title.into(), answer_id.into_inner().into()],
                        txn,
                    )
                    .await?;
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }
}
