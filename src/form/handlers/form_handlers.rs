use crate::form::domain::Form;
use crate::form::handlers::domain_for_user_input::{raw_form::RawForm, raw_form_id::RawFormId};
use crate::form::infrastructure::create_form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use derive_getters::Getters;
use std::sync::{Arc, Mutex};
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Getters)]
pub struct FormHandlers {
    forms: Mutex<Vec<Form>>,
}

pub async fn create_form_handler(
    State(forms): State<Arc<FormHandlers>>,
    Json(request_form): Json<RawForm>,
) -> impl IntoResponse {
    println!("create form handler.");
    match create_form(request_form, forms).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(err) => {
            println!("データベースエラー:{}", err.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, "db error")
        }
    }
}

pub async fn delete_form_handler(info: RawFormId) {
    // println!("{:?}", info.id());
    // delete_form(info);
    // HttpResponse::Ok().body("Success")
    todo!()
}
