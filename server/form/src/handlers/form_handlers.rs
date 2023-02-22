use crate::domain::Form;
use crate::handlers::models::{RawForm, RawFormId};
use crate::infrastructure::create_form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tracing::log::error;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct FormHandlers {
    pub forms: Mutex<Vec<Form>>,
}

pub async fn create_form_handler(
    State(forms): State<Arc<FormHandlers>>,
    Json(request_form): Json<RawForm>,
) -> impl IntoResponse {
    match create_form(request_form, forms).await {
        Ok(form_id) => (StatusCode::CREATED, json!(form_id).to_string()),
        Err(err) => {
            error!("create_form_handler: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "db error".to_owned())
        }
    }
}

pub async fn delete_form_handler(_info: RawFormId) {
    // println!("{:?}", info.id());
    // delete_form(info);
    // HttpResponse::Ok().body("Success")
    todo!()
}
