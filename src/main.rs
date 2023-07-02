use std::collections::BTreeMap;
use jwt::SignWithKey;
use sha2::Sha512;
use websocket::futures::Stream;
use websocket::url;

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
    let upbit = upbit::UPBitAccount::new("qynTp0pDQGLkYl4VmoZax9ftGjQNe5YwLpJ7X4fm", "0ikxDGSb4XkwndGIYhd1yNzE0jUji1lQHqEPxWfx");
    match upbit.get_tickers_sortby_volume().await {
        Ok(values) => {
            for (c, b) in values {
                println!("{}: {}",c,b);
            }
        }
        Err(e) => eprintln!("{}", e)
    }

    let mut upbit_websocket = upbit::websocket::UPBitWebSocket::new(&upbit).await;

    loop {
        upbit_websocket.get_realtime_data();
    }

}