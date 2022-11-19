use actix_web::{App, HttpResponse, HttpServer, post, Responder};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
    }).bind(("127.0.0.1", 9000))?
        .run()
        .await
}

#[post("/api/form/report/{contents}")]
async fn report() -> impl Responder {
    todo!();
}

#[post("/api/form/bug-report/{contents}")]
async fn bug_report() -> impl Responder {
    todo!();
}

#[post("/api/form/request/{contents}")]
async fn request() -> impl Responder {
    todo!()
}

#[post("/api/form/can-mod-use/{contents}")]
async fn can_mod_use() -> impl Responder {
    todo!()
}

#[post("/api/form/exp-overflow/{contents}")]
async fn exp_overflow() -> impl Responder {
    todo!()
}

#[post("/api/form/blacklist/{contents}")]
async fn blacklist() -> impl Responder {
    todo!()
}

#[post("/api/form/general-inquiry/{contents}")]
async fn general_inquiry() -> impl Responder {
    todo!()
}
