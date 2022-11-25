use crate::database::connection;
use actix_web::{App, HttpServer};

use form::handlers::create_form_handler;
use form::handlers::delete_form_handler;
use migration::MigratorTrait;

mod database;
mod form;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _connection = connection::database_connection().await;
    migration::Migrator::up(&_connection, None).await.unwrap();
    HttpServer::new(|| {
        App::new()
            .service(create_form_handler)
            .service(delete_form_handler)
    })
    .bind(("0.0.0.0", 9000))?
    .run()
    .await
}

// #[cfg(test)]
// mod tests {
//     use crate::forms::handlers::domain_for_user_input::raw_form::RawForm;
//     use actix_web::{dev::Service, http, test, App};
//
//     use super::*;
//
//     #[actix_web::test]
//     async fn test_index() {
//         let app = test::init_service(App::new().service(create_form_handler)).await;
//
//         let req = test::TestRequest::post()
//             .uri("/api/forms/create")
//             .set_json(&RawForm {
//                 form_name: "test1".to_owned(),
//                 form_id: 1,
//             })
//             .to_request();
//         let resp = app.call(req).await.unwrap();
//
//         assert_eq!(resp.status(), http::StatusCode::OK);
//     }
// }
