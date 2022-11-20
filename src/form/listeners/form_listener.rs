use crate::form::domain::{create_form, Form};
use actix_web::{post, web::Json, HttpResponse, Responder};

#[post("/api/form/create")]
pub async fn create_form_listener(info: Json<Form>) -> impl Responder {
    println!("create_form_listener");
    println!("{:?}", info.0.form_titles());
    println!("{:?}", info.0.form_id());
    create_form(info.0);
    HttpResponse::Ok().body("OK")
}
