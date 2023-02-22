use crate::handlers::FormHandlers;
use database::connection::database_connection;
use database::entities::{form_choices, form_questions, forms};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, TransactionTrait};

use crate::domain::Form;
use errors::anywhere;
use errors::error_definitions::FormInfraError;
use itertools::Itertools;

use crate::handlers::models::{RawForm, RawFormId};
use std::sync::Arc;

/// formを生成する
pub async fn create_form(form: RawForm, handler: Arc<FormHandlers>) -> anywhere::Result<RawFormId> {
    let connection = database_connection().await;

    let txn = connection.begin().await?;

    let form_id = forms::ActiveModel {
        id: ActiveValue::NotSet,
        name: Set(form.form_name().to_owned()),
    }
    .insert(&txn)
    .await?
    .id;

    let questions = form
        .questions()
        .iter()
        .map(|question| {
            let form_questions = form_questions::ActiveModel {
                question_id: ActiveValue::NotSet,
                form_id: Set(form_id),
                title: Set(question.title().to_owned()),
                description: Set(question.description().to_owned()),
                answer_type: Set(question.question_type().to_string()),
            };

            let form_choices = question.choices().clone().map(|choices| {
                choices
                    .iter()
                    .map(|choice| form_choices::ActiveModel {
                        id: ActiveValue::NotSet,
                        question_id: form_questions.question_id.clone(),
                        choice: Set(choice.to_string()),
                    })
                    .collect_vec()
            });

            (form_questions, form_choices)
        })
        .collect_vec();

    let form_questions = questions
        .iter()
        .map(|(question, _)| question.clone())
        .collect_vec();

    form_questions::Entity::insert_many(form_questions)
        .exec(&txn)
        .await?;

    let form_choices = questions
        .iter()
        .map(|(_, choices)| choices.clone())
        .collect::<Option<Vec<_>>>()
        .map(|choices| choices.iter().flatten().cloned().collect_vec());

    if form_choices.is_some() {
        form_choices::Entity::insert_many(form_choices.unwrap())
            .exec(&txn)
            .await?;
    }

    {
        let mut forms = handler
            .forms
            .lock()
            .map_err(|_| FormInfraError::MutexLockFailed)?;

        forms.push(form.to_form(form_id));
    }

    txn.commit().await?;

    Ok(RawFormId::builder().id(form_id).build())
}

/// 作成されているフォームの取得
/// 取得に失敗した場合はpanicします
pub async fn fetch_forms() -> Vec<Form> {
    todo!()
    // let _connection = database_connection().await;
    //
    // let txn = _connection
    //     .begin()
    //     .await?;
    //
    // forms::Entity::find().join(
    //     JoinType::InnerJoin,
    //     forms::Relation::def()
    // ).join(
    //     JoinType::InnerJoin,
    //     form_questions::Relation::def()
    // ).into_model::<>()
    //
    // forms::Entity::find()
    //     .find_with_related([form_questions::Entity, form_choices::Entity])
    //     .all(&txn)
    //     .await?
    //     .iter()
    //     .map(|(form_info, questions)| {
    //         let form_name = FormName::builder().name(form_info.clone().name).build();
    //         let form_id = FormId::builder().form_id(form_info.id).build();
    //         let questions = questions
    //             .iter()
    //             .map(|question| {
    //                 let question_info = question.clone();
    //                 match from_string(question_info.answer_type) {
    //                     Some(question_type) => Question::builder()
    //                         .title(question_info.title)
    //                         .description(question_info.description)
    //                         .question_type(question_type)
    //                         .choices(question_info.choices.map(|choice| {
    //                             // choice.split(',').map(|s| s.to_string()).collect()
    //                         }))
    //                         .build(),
    //                     None => panic!("question_typeのデシリアライズに失敗しました"),
    //                 }
    //             })
    //             .collect::<Vec<Question>>();
    //         Form::builder()
    //             .name(form_name)
    //             .id(form_id)
    //             .questions(questions)
    //             .build()
    //     })
    //     .collect::<Vec<Form>>()
}

/// formを削除する
pub fn delete_form(_form_id: RawFormId) -> bool {
    todo!()
    // let connection = &mut database_connection();
    // let transaction: Result<(), Error> = connection.transaction(|connection| {
    //     sql_query("DELETE FROM seichi_portal.forms WHERE id = ?")
    //         .bind::<Integer, _>(_form_id.id())
    //         .execute(connection)?;
    //     sql_query(format!("DROP TABLE forms.{}", _form_id.id())).execute(connection)?;
    //
    //     Ok(())
    // });
    //
    // transaction.is_ok()
}
