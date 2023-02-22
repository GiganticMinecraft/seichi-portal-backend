use crate::handlers::FormHandlers;
use database::connection::database_connection;
use database::entities::forms;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ActiveValue, TransactionTrait};

use crate::domain::{Form, FormId, FormName};
use errors::anywhere;
use errors::error_definitions::FormInfraError;

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
            name: Set(form_name.clone().0),
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
