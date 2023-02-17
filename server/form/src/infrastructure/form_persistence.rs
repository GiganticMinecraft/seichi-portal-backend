use crate::handlers::domain_for_user_input::raw_form::RawForm;
use crate::handlers::domain_for_user_input::raw_form_id::RawFormId;
use crate::handlers::FormHandlers;
use database::connection::database_connection;
use database::entities::{form_questions, forms};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, EntityTrait, QuerySelect, QueryTrait, TransactionTrait};
use std::borrow::Borrow;

use crate::domain::{from_string, Form, FormId, FormName, Question};
use errors::error_definitions::Error;
use std::sync::Arc;
use errors::anywhere;

/// formを生成する
pub async fn create_form(form: RawForm, handler: Arc<FormHandlers>) -> anywhere::Result<RawFormId> {
    let connection = database_connection().await;

    let txn = connection.begin().await.map_err(|err| {
        println!("{}", err);
        Error::DbTransactionConstructionError
    })?;

    let form_id = forms::ActiveModel {
        id: Default::default(),
        name: Set(form.form_name().to_owned()),
    }
    .insert(&txn)
    .await
    .map_err(|err| {
        println!("{}", err);
        Error::SqlExecutionError
    })?
    .id;

    let questions = form
        .questions()
        .iter()
        .map(|question| form_questions::ActiveModel {
            question_id: Default::default(),
            form_id: Set(form_id),
            title: Set(question.title().to_owned()),
            description: Set(question.description().to_owned()),
            answer_type: Set(question.question_type().to_string()),
            choices: Set(question.choices().clone().map(|choices| choices.join(","))),
        })
        .collect::<Vec<form_questions::ActiveModel>>();

    form_questions::Entity::insert_many(questions)
        .exec(&txn)
        .await
        .map_err(|err| {
            println!("{}", err);
            Error::SqlExecutionError
        })?;

    match handler.forms.lock() {
        Err(err) => {
            println!("{}", err);
            return Err(Error::MutexCanNotUnlock);
        }
        Ok(mut forms) => {
            forms.push(form.to_form(form_id));
            forms
                .iter()
                .for_each(|form| println!("{}", form.name().name()))
        }
    }

    txn.commit().await.map_err(|err| {
        println!("{}", err);
        Error::DbTransactionConstructionError
    })?;

    Ok(RawFormId::builder().id(form_id).build())
}

/// 作成されているフォームの取得
/// 取得に失敗した場合はpanicします
pub async fn fetch_forms() -> Vec<Form> {
    let _connection = database_connection().await;

    let txn = _connection
        .begin()
        .await
        .unwrap_or_else(|_| panic!("データベースのトランザクション確立に失敗しました。"));

    forms::Entity::find()
        .find_with_related(form_questions::Entity)
        .all(&txn)
        .await
        .unwrap_or_else(|_| panic!("フォーム情報の取得に失敗しました"))
        .iter()
        .map(|(form_info, questions)| {
            let form_name = FormName::builder().name(form_info.clone().name).build();
            let form_id = FormId::builder().form_id(form_info.id).build();
            let questions =
                questions
                    .iter()
                    .map(|question| {
                        let question_info = question.clone();
                        match from_string(question_info.answer_type) {
                            Some(question_type) => Question::builder()
                                .title(question_info.title)
                                .description(question_info.description)
                                .question_type(question_type)
                                .choices(question_info.choices.map(|choice| {
                                    choice.split(',').map(|s| s.to_string()).collect()
                                }))
                                .build(),
                            None => panic!("question_typeのデシリアライズに失敗しました"),
                        }
                    })
                    .collect::<Vec<Question>>();
            Form::builder()
                .name(form_name)
                .id(form_id)
                .questions(questions)
                .build()
        })
        .collect::<Vec<Form>>()
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
