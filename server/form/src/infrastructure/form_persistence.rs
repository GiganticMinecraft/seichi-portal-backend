use crate::handlers::FormHandlers;
use database::connection::database_connection;
use database::entities::{form_choices, form_questions, forms};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, TransactionTrait};

use crate::domain::{question_type_from_string, Form, FormId, FormName, Question};
use errors::anywhere;
use errors::error_definitions::FormInfraError;

use itertools::Itertools;
use std::sync::Arc;

/// formを作成する
pub async fn create_form(
    form_name: FormName,
    handler: Arc<FormHandlers>,
) -> anywhere::Result<FormId> {
    let connection = database_connection().await;

    let txn = connection.begin().await?;

    let form_id = FormId(
        forms::ActiveModel {
            id: ActiveValue::NotSet,
            name: Set(form_name.clone().name().to_owned()),
        }
        .insert(&txn)
        .await?
        .id,
    );

    let form = Form::builder()
        .id(form_id.clone())
        .name(form_name.clone())
        .questions(vec![])
        .build();

    {
        let mut forms = handler
            .forms
            .lock()
            .map_err(|_| FormInfraError::MutexLockFailed)?;

        forms.push(form);
    }

    txn.commit().await?;

    Ok(form_id.clone())
}

/// 作成されているフォームの取得
/// 取得に失敗した場合はpanicします
pub async fn fetch_forms() -> anywhere::Result<Vec<Form>> {
    let connection = database_connection().await;

    let txn = connection.begin().await?;

    let persisted_forms = forms::Entity::find().all(&txn).await?;
    let persisted_questions = form_questions::Entity::find().all(&txn).await?;
    let persisted_choices = form_choices::Entity::find().all(&txn).await?;

    let forms = persisted_forms
        .into_iter()
        .map(|form| {
            let target_question = persisted_questions
                .clone()
                .into_iter()
                .filter_map(|question| {
                    question_type_from_string(question.answer_type)
                        .filter(|_| question.form_id == form.id)
                        .map(|question_type| {
                            let choices = persisted_choices
                                .iter()
                                .filter_map(|choice| {
                                    let is_same_question =
                                        choice.question_id == question.question_id;
                                    is_same_question.then(|| choice.choice.clone())
                                })
                                .collect_vec();

                            Question::builder()
                                .title(question.title)
                                .description(question.description)
                                .question_type(question_type)
                                .choices(choices)
                                .build()
                        })
                })
                .collect_vec();

            Form::builder()
                .name(FormName::builder().name(form.name).build())
                .id(FormId(form.id))
                .questions(target_question)
                .build()
        })
        .collect_vec();

    Ok(forms)
}
