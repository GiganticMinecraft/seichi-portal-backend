use crate::database::connection::database_connection;
use crate::database::entities::{form_questions, forms};
use crate::form::domain::Form;
use crate::form::handlers::domain_for_user_input::raw_form::RawForm;
use crate::form::handlers::domain_for_user_input::raw_form_id::RawFormId;
use crate::form::handlers::FormHandlers;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, TransactionError, TransactionTrait};
use std::borrow::Borrow;
use std::mem;
use std::sync::{Arc, Mutex};

/// formを生成する
pub async fn create_form(
    form: RawForm,
    handler: Arc<FormHandlers>,
) -> Result<(), TransactionError<DbErr>> {
    let connection = database_connection().await;
    connection
        .transaction::<_, (), DbErr>(|txn| {
            Box::pin(async move {
                let form_id = forms::ActiveModel {
                    id: Default::default(),
                    name: Set(form.form_name().to_owned().into()),
                }
                .insert(txn)
                .await?
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
                    .exec(txn)
                    .await?;

                let mut handler_value = handler.forms().lock().unwrap();
                handler_value.push(form.to_form(form_id));

                println!("{}", handler_value.first().unwrap().name().name());

                Ok(())
            })
        })
        .await?;

    Ok(())
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
