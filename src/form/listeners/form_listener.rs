use crate::form::domain::Form;
use actix_web::{post, web::Json, HttpResponse, Responder};

#[post("/api/form/create")]
async fn create_form(_info: Json<Form>) -> impl Responder {
    HttpResponse::Ok()
}
