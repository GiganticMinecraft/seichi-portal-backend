use crate::form::controllers::raw_form::RawForm;
use crate::form::domain::create_form;
use actix_web::{post, web::Json, HttpResponse, Responder};

#[post("/api/form/create")]
pub async fn create_form_listener(info: Json<RawForm>) -> impl Responder {
    println!("{:?}", info.0.form_titles());
    println!("{:?}", info.0.form_id());
    create_form(info.0);
    HttpResponse::Ok().body("OK")
}
