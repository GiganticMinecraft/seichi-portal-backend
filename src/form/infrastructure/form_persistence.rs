use crate::database::connection::database_connection;
use crate::database::entities::{form_questions, forms};
use crate::errors::error_definitions::Error;
use crate::form::domain::FormId;
use crate::form::handlers::domain_for_user_input::raw_form::RawForm;
use crate::form::handlers::domain_for_user_input::raw_form_id::RawFormId;
use crate::form::handlers::FormHandlers;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, TransactionError, TransactionTrait};
use std::borrow::Borrow;
use std::sync::Arc;

/// formを生成する
pub async fn create_form(form: RawForm, handler: Arc<FormHandlers>) -> Result<RawFormId, Error> {
    let connection = database_connection().await;

    let txn = connection.begin().await.map_err(|err| {
        println!("{}", err);
        Error::DbTransactionConstructionError
    })?;

    let form_id = forms::ActiveModel {
        id: Default::default(),
        name: Set(form.form_name().to_owned().into()),
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

    // let mut forms = handler.forms.lock().map_err(|err| {
    //     println!("{}", err);
    //     Error::MutexCanNotUnlock
    // })?;
    //
    // forms.push(form.to_form(form_id));

    match handler.forms.lock() {
        Err(err) => {
            println!("{}", err);
            return Err(Error::MutexCanNotUnlock);
        }
        Ok(mut value) => value.push(form.to_form(form_id)),
    }

    txn.commit().await.map_err(|err| {
        println!("{}", err);
        Error::DbTransactionConstructionError
    })?;

    Ok(RawFormId::builder().id(form_id).build())
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
