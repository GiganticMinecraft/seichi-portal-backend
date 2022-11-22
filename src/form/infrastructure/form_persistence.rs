use crate::database::connection::database_connection;
use crate::form::controllers::raw_form_id::RawFormId;
use crate::form::domain::Form;
use diesel::sql_types::VarChar;
use diesel::{sql_query, QueryResult, RunQueryDsl};

/// formを生成する
pub fn create_form(_form: Form) -> QueryResult<usize> {
    let mut connection = database_connection();
    sql_query("INSERT INTO forms.forms (name) VALUES (?)")
        .bind::<VarChar, _>(_form.form_name().name())
        .execute(&mut connection)
}

/// formを削除する
pub fn delete_form(_form_id: RawFormId) {}
