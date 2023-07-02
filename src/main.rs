use std::collections::BTreeMap;
use jwt::SignWithKey;
use polars::export::arrow::compute::if_then_else::if_then_else;
use polars::prelude::IntoLazy;
use polars::series::ops::NullBehavior;
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
    use polars::prelude::*;
    let upbit_floweryclover = upbit::restful::UPBitAccount::new("qynTp0pDQGLkYl4VmoZax9ftGjQNe5YwLpJ7X4fm", "0ikxDGSb4XkwndGIYhd1yNzE0jUji1lQHqEPxWfx");
    // match upbit_floweryclover.get_all_balances().await {
    //     Ok(values) => {
    //         for (c, b) in values {
    //             println!("{}: {}",c,b);
    //         }
    //     }
    //     Err(e) => eprintln!("{}", e)
    // }
    let mut upbit_socket = upbit::UPBitSocket::new();
    // match upbit_socket.get_tickers_sortby_volume().await {
    //     Ok(values) => {
    //         for (c, b) in values {
    //             println!("{}: {}",c,b);
    //         }
    //     }
    //     Err(e) => eprintln!("{}", e)
    // }

    let df = upbit_socket.get_recent_market_data("KRW-BTC", 200).await.unwrap();
    println!("{}", df);

    let diff = df.lazy().select([
        col("timestamp"),
        col("trade_price").diff(1, NullBehavior::Ignore).alias("diff"),
    ]).collect().unwrap();

    let au_ad = diff.clone().lazy().select([
        when(col("diff").lt(lit(0)))
            .then(lit(0))
            .otherwise(col("diff"))
            .ewm_mean(EWMOptions::default().and_com(14.0).and_min_periods(14))
            .alias("au"),
        when(col("diff").gt(lit(0)))
            .then(lit(0))
            .otherwise(col("diff"))
            .abs()
            .ewm_mean(EWMOptions::default().and_com(14.0).and_min_periods(14))
            .alias("ad"),
    ]).collect().unwrap();

    let rsi = au_ad.lazy().select([
        (lit(100.0) - (lit(100.0) / (lit(1.0)+(col("au")/col("ad")) )))
            .alias("rsi")
    ]).collect().unwrap();
    println!("{}", rsi);
}