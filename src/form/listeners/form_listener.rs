use crate::form::domain::{create_form, Form};
use actix_web::{post, web::Json, HttpResponse, Responder};

#[post("/api/form/create")]
async fn create_form_listener(_info: Json<Form>) -> impl Responder {
    create_form(_info.0);
    HttpResponse::Ok()
}
