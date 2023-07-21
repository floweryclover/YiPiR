use crate::upbit::api::{CandleUnit, get_all_tickers, guaranteed_get_candle_data, sell_market_order, get_balance_of, buy_market_order};
use crate::upbit::ops::RsiDivergenceCheckMode;
use crate::upbit::response::{CandleData, CandleDataOperation};

mod api;
mod response;
mod ops;

#[derive(Clone)]
pub struct UpbitAccount {
    access_key: String,
    secret_key: String,
}

impl UpbitAccount {
    pub fn new(access_key: String, secret_key: String) -> UpbitAccount {
        UpbitAccount { access_key, secret_key }
    }
}

pub async fn spawn_yipir_upbit_service() -> tokio::task::JoinHandle<()> {
    use tokio::{task, time};
    use tokio::time::Duration;

    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(1));
        let upbit_account = UpbitAccount::new(
            String::from(" UPBit Access Key"),
            String::from(" UPBit Secret Key "));

         loop {
             interval.tick().await;
             let all_tickers = get_all_tickers().await;
             let mut candle_datas = Vec::new();
             for ticker in all_tickers {
                 candle_datas.push(guaranteed_get_candle_data(ticker.as_str(), CandleUnit::Min1, 200).await);
             }

             //구매 점수 높은 종목을 찾아 구매 시행
             candle_datas
                 .iter()
                 .for_each(
                     |data| {
                         if data.check_rsi_divergence(&RsiDivergenceCheckMode::Minpoint, &30.0, &5) // 5개 데이터 이내 RSI 다이버전스 발생
                             && data.get_last_price() < data.get_ewm_mean() // 현재 가격이 평균보다 낮음
                         {
                             let ticker = data[0].market.clone();
                             let cloned_account = upbit_account.clone();
                             task::spawn(async move {
                                 if let Some(krw) = get_balance_of(&cloned_account, "KRW").await {
                                     if let None = get_balance_of(&cloned_account, &ticker).await {
                                         buy_market_order(&cloned_account, &ticker, krw * 0.2).await;
                                     }
                                 }
                             });
                         }
                     }
                 );

             // 판매 점수 높은 종목을 찾아 판매 시행
             candle_datas
                 .iter()
                 .for_each(
                     |data| {
                         if data.check_rsi_divergence(&RsiDivergenceCheckMode::Peak, &70.0, &5) // 5개 데이터 이내 RSI 다이버전스 발생
                             || data.check_rsi_breaking_peak(&4, &70.0) // RSI 꺾임 발생
                             || data.get_rsi() > 60.0 && data.get_last_price() < data.get_ewm_mean() // RSI가 올랐는데도 가격이 오르지 않았으면 가망이 없는 종목이라 판단
                         {
                             let ticker = data[0].market.clone();
                             let cloned_account = upbit_account.clone();

                             task::spawn(async move {
                                 if let Some(_) = get_balance_of(&cloned_account, &ticker).await {
                                     let _ = sell_market_order(&cloned_account, &ticker, 100.0).await;
                                 }
                             });
                         }
                     }
                 );
         }
    })
}
