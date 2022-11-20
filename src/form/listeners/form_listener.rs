use crate::form::controllers::raw_form::RawForm;
use crate::form::domain::{create_form, delete_form, FormId};
use actix_web::{post, web::Json, HttpResponse, Responder};

#[post("/api/form/create")]
pub async fn create_form_listener(info: Json<RawForm>) -> impl Responder {
    println!("{:?}", info.form_titles());
    println!("{:?}", info.form_id());
    create_form(info);
    HttpResponse::Ok().body("Success")
}

#[post("/api/form/delete")]
pub async fn delete_form_listener(info: Json<FormId>) -> impl Responder {
    println!("{:?}", info.form_id);
    delete_form(info.form_id);
    HttpResponse::Ok().body("Success")
}
