mod upbit;
//
// use std::sync::Mutex;
// use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
// use crate::upbitq::UPBit;
//
// struct UPBitState {
//     upbitq: Mutex<UPBit>,
// }
//
// #[get("/balance")]
// async fn get_balance(data: web::Data<UPBitState>) -> impl Responder {
//     let balance = data.upbitq.lock().unwrap().balance();
//     HttpResponse::Ok().body(format!("Why hangul crashes? {}", balance))
// }
//
// #[get("/sell")]
// async fn sell_coin(data: web::Data<UPBitState>) -> impl Responder {
//     let mut upbitq = data.upbitq.lock().unwrap();
//     upbitq.sell();
//     HttpResponse::Ok().body("Sold some coin")
// }
//
// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     HttpServer::new(|| {
//         App::new()
//             .service(web::scope("/api")
//                 .app_data(web::Data::new(UPBitState { upbitq:Mutex::new(UPBit::new()) }))
//                 .service(get_balance)
//                 .service(sell_coin)
//             )
//     })
//         .bind(("127.0.0.1", 80))?
//         .run()
//         .await
// }

#[tokio::main]
async fn main() {
    let upbit_service = upbit::UPBitService::new();
    let _ = upbit_service.run().await.await;
}