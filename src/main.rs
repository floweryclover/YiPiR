use crate::upbit::spawn_yipir_upbit_service;

mod upbit;

#[tokio::main]
async fn main() {
    let _ = spawn_yipir_upbit_service().await.await;
}