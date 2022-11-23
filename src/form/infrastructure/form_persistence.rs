use crate::database::connection::database_connection;
use crate::form::handlers::domain_for_user_input::raw_form::RawForm;
use crate::form::handlers::domain_for_user_input::raw_form_id::RawFormId;
use diesel::result::Error;
use diesel::sql_types::{Integer, Nullable, Text, VarChar};
use diesel::{sql_query, Connection, MysqlConnection, QueryResult, RunQueryDsl};

/// formを生成する
pub fn create_form(form: RawForm) -> bool {
    let connection: &mut MysqlConnection = &mut database_connection();

    let transaction = connection.transaction(|connection| {
        let is_form_inserted = sql_query("INSERT INTO seichi_portal.forms (name) VALUES (?)")
            .bind::<VarChar, _>(form.form_name())
            .execute(connection)
            .is_ok();

        let created_form_id = sql_query("SELECT id FROM seichi_portal.forms WHERE name = ?")
            .bind::<VarChar, _>(form.form_name())
            .get_result::<RawFormId>(connection)
            .unwrap();

        // NOTE: ここのid埋め込みは自動生成の数字なのでそのまま埋め込んで良い
        let is_success_create_table = sql_query(format!(
            r"CREATE TABLE forms.{} (
            id INT AUTO_INCREMENT,
            title VARCHAR(100),
            description VARCHAR(300),
            type VARCHAR(10),
            choices TEXT,
            PRIMARY KEY(id)
        )
        ",
            created_form_id.id()
        ))
        .execute(connection)
        .is_ok();

        let mut insert_state = form.questions().iter().map(|question| {
            let choices = question.choices().clone().map(|choices| choices.join(","));
            sql_query(format!(
                r"INSERT INTO forms.{} (title, description, type, choices)
                VALUES (?, ?, ?, ?)
            ",
                created_form_id.id()
            ))
            .bind::<VarChar, _>(question.title())
            .bind::<VarChar, _>(question.description())
            .bind::<VarChar, _>(question.question_type().to_string())
            .bind::<Nullable<Text>, _>(choices)
            .execute(connection)
            .is_ok()
        });

        let database_process_state =
            is_form_inserted && is_success_create_table && insert_state.all(|rs| rs == true);

        if database_process_state {
            Ok(())
        } else {
            Err(Error::RollbackTransaction)
        }
    });

    transaction,is_ok()
}

/// formを削除する
pub fn delete_form(_form_id: RawFormId) -> QueryResult<usize> {
    let mut connection = database_connection();
    sql_query("DELETE FROM seichi_portal.forms WHERE id = ?")
        .bind::<Integer, _>(_form_id.id())
        .execute(&mut connection)
}
