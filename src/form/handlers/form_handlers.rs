use crate::form::handlers::domain_for_user_input::{raw_form::RawForm, raw_form_id::RawFormId};
use crate::form::infrastructure::{create_form, delete_form};
use actix_web::{post, web::Json, HttpResponse, Responder};

#[post("/api/form/create")]
pub async fn create_form_handler(info: Json<RawForm>) -> impl Responder {
    let form = info.0;
    println!("{:?}", form.form_name());
    create_form(form);
    HttpResponse::Ok().body("Success")
}

#[post("/api/form/delete")]
pub async fn delete_form_handler(info: Json<RawFormId>) -> impl Responder {
    println!("{:?}", info.0.form_id());
    delete_form(info.0);
    HttpResponse::Ok().body("Success")
}
