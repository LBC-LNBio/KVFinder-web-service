use actix_web::{error, web, App, HttpRequest, HttpResponse, HttpServer};
use kv;

fn json_error_handler(err: error::JsonPayloadError, _req: &HttpRequest) -> error::Error {
    let msg = String::from("Please update your plugin");
    let resp = HttpResponse::BadRequest().body(msg);
    error::InternalError::from_response(err, resp).into()
}

fn main() {
    println!("KVFinder webserver started");

    // job timeout 30 minutes, expires after 1 day
    kv::webserver::create_ocypod_queue("kvfinder", "30m", "1d", 0);

    HttpServer::new(|| {
        App::new()
            .data(
                web::JsonConfig::default()
                    .limit(1_000_000)
                    .error_handler(json_error_handler),
            )
            .route("/", web::get().to(kv::webserver::hello))
            .route("/{id}", web::get().to(kv::webserver::ask))
            .route("/create", web::post().to(kv::webserver::create))
    })
    .bind("0.0.0.0:8081")
    .expect("Cannot bind to port 8081")
    .run()
    .unwrap();
}
