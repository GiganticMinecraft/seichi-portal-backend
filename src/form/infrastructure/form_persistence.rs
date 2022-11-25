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
                    let b = Value::String(Some(Box::from(question.title().to_owned())));
                    let c = Value::String(Some(Box::from(question.description().to_owned())));
                    let d = Value::String(Some(Box::from(
                        question.question_type().to_owned().to_string().to_owned(),
                    )));
                    let f = question
                        .choices()
                        .clone()
                        .map(|choices| Box::from(choices.join(",")));
                    let e = Value::String(f);

                    vec![b, c, d, e]
                })
                .collect::<Vec<Vec<Value>>>();

            // serialized_questions.iter().for_each(|question| async {
            //     txn.execute(Statement::from_sql_and_values::<Vec<Value>>(
            //         MySql,
            //         &*format!(
            //             r"INSERT INTO forms.{} (title, description, type, choices)
            //         VALUES (?, ?, ?, ?)",
            //             created_form_id
            //         ),
            //         question.to_owned().into(),
            //     ))
            // });

            Ok(())
        })
    });

    transaction.await?;

    Ok(())

    // let transaction: Result<(), Error> = connection.transaction(|connection| {
    //     sql_query("INSERT INTO seichi_portal.forms (name) VALUES (?)")
    //         .bind::<VarChar, _>(form.form_name())
    //         .execute(connection)?;
    //
    //     let created_form_id =
    //         sql_query("SELECT LAST_INSERT_ID() AS id").get_result::<RawFormId>(connection)?;
    //
    //     // NOTE: ここのid埋め込みは自動生成の数字なのでそのまま埋め込んで良い
    //     sql_query(format!(
    //         r"CREATE TABLE forms.{} (
    //         id INT AUTO_INCREMENT,
    //         title VARCHAR(100) NOT NULL,
    //         description VARCHAR(300) NOT NULL,
    //         type VARCHAR(10) NOT NULL,
    //         choices TEXT,
    //         PRIMARY KEY(id)
    //     )
    //     ",
    //         created_form_id.id()
    //     ))
    //     .execute(connection)?;
    //
    //     let mut insert_query = form.questions().iter().map(|question| {
    //         let choices = question.choices().clone().map(|choices| choices.join(","));
    //         sql_query(format!(
    //             r"INSERT INTO forms.{} (title, description, type, choices)
    //             VALUES (?, ?, ?, ?)
    //         ",
    //             created_form_id.id()
    //         ))
    //         .bind::<VarChar, _>(question.title())
    //         .bind::<VarChar, _>(question.description())
    //         .bind::<VarChar, _>(question.question_type().to_string())
    //         .bind::<Nullable<Text>, _>(choices)
    //         .execute(connection)
    //         .is_ok()
    //     });
    //
    //     if insert_query.all(|query| query == true) {
    //         Ok(())
    //     } else {
    //         Err(Error::RollbackTransaction)
    //     }
    // });
    //
    // transaction.is_ok()
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
