use actix_web::{App, HttpServer};
use form::listeners::create_form_listener;

mod form;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("test");
    HttpServer::new(|| App::new().service(create_form_listener))
        .bind(("127.0.0.1", 9000))?
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use crate::form::domain::{Form, FormId, FormTitle};
    use actix_web::{body::to_bytes, dev::Service, http, test, web, App};

    use super::*;

    #[actix_web::test]
    async fn test_index() {
        let app = test::init_service(App::new().service(create_form_listener)).await;

        let req = test::TestRequest::post()
            .uri("/api/form/create")
            .set_json(&Form {
                form_titles: vec![FormTitle {
                    title: "test1".parse().unwrap(),
                }],
                form_id: FormId { form_id: 1 },
            })
            .to_request();
        let resp = app.call(req).await.unwrap();

        assert_eq!(resp.status(), http::StatusCode::OK);
    }
}
