use super::entities::forms::Entity as Forms;
use crate::database::connection::database_connection;
use crate::form::handlers::domain_for_user_input::raw_form::RawForm;
use crate::form::handlers::domain_for_user_input::raw_form_id::RawFormId;
use sea_orm::DatabaseBackend::MySql;
use sea_orm::{ConnectionTrait, DbErr, Statement, TransactionError, TransactionTrait, Value};

/// formを生成する
pub async fn create_form(form: RawForm) -> Result<(), TransactionError<DbErr>> {
    let connection = database_connection().await;
    let transaction = connection.transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            txn.execute(Statement::from_sql_and_values(
                MySql,
                &*"INSERT INTO seichi_portal.forms (name) VALUES (?)".to_owned(),
                vec![form.form_name().to_owned().into()],
            ))
            .await?;

            let created_form_id = txn
                .query_one(Statement::from_string(
                    MySql,
                    "SELECT CAST(LAST_INSERT_ID() AS SIGNED) AS id".to_owned(),
                ))
                .await?
                .unwrap()
                .try_get::<i32>("", "id")?;

            txn.execute(Statement::from_string(
                MySql,
                format!(
                    r"CREATE TABLE forms.{} (
                    id INT AUTO_INCREMENT,
                    title VARCHAR(100) NOT NULL,
                    description VARCHAR(300) NOT NULL,
                    type VARCHAR(10) NOT NULL,
                    choices TEXT,
                    PRIMARY KEY(id)
                    )",
                    created_form_id
                ),
            ))
            .await?;

            let serialized_questions = form
                .questions()
                .iter()
                .map(|question| {
                    let title = Value::String(Some(Box::from(question.title().to_owned())));
                    let description =
                        Value::String(Some(Box::from(question.description().to_owned())));
                    let question_type = Value::String(Some(Box::from(
                        question.question_type().to_owned().to_string().to_owned(),
                    )));
                    let choices_opt = question
                        .choices()
                        .clone()
                        .map(|choices| Box::from(choices.join(",")));
                    let choices = Value::String(choices_opt);

                    vec![title, description, question_type, choices]
                })
                .collect::<Vec<Vec<Value>>>();

            serialized_questions.iter().for_each(|question| {
                async {
                    txn.execute(Statement::from_sql_and_values::<Vec<Value>>(
                        MySql,
                        &*format!(
                            r"INSERT INTO forms.{} (title, description, type, choices)
                    VALUES (?, ?, ?, ?)",
                            created_form_id
                        ),
                        question.to_owned().into(),
                    ))
                };
            });

            Ok(())
        })
    });

    transaction.await?;

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
