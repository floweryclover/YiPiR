use polars::prelude::*;
use polars::series::ops::NullBehavior;
use polars_io::prelude::*;
use serde_json::error::Category::Data;
use crate::upbit::{UPBitSocket, UPBitError, CallMethod, request_get, request_post, generate_request_body, response_to_json};

impl UPBitSocket {
    pub async fn get_tickers_sortby_volume(&self) -> Result<Vec<(String, f64)>, UPBitError> {
        let tickers = match self.get_all_available_tickers().await {
            Ok(values) => values,
            Err(e) => return Err(e)
        };

        let mut url = String::from("https://api.upbit.com/v1/ticker?markets=");
        for ticker in tickers {
            url.push_str(&ticker);
            url.push_str(",");
        }
        url.pop();

        match request_get(&self.reqwest_client, &url, CallMethod::Public).await {
            Ok(json_array) => {
                let mut return_vec = Vec::new();
                for item in json_array {
                    return_vec.push((String::from(item["market"].as_str().unwrap()), item["trade_price"].as_f64().unwrap() * item["acc_trade_volume_24h"].as_f64().unwrap()));
                }
                return_vec.sort_by(|(_, va), (_, vb)| vb.partial_cmp(va).unwrap());
                return Ok(return_vec);
            }
            Err(e) => Err(e)
        }
    }

    pub async fn get_all_available_tickers(&self) -> Result<Vec<String>, UPBitError> {
        match request_get(&self.reqwest_client, "https://api.upbit.com/v1/market/all", CallMethod::Public).await {
            Ok(json_array) => {
                let ticker_list: Vec<String> = json_array.iter().filter(|j| j["market"].as_str().unwrap().contains("KRW")).map(|j| String::from(j["market"].as_str().unwrap())).collect();
                if ticker_list.is_empty() {
                    return Err(UPBitError::FailedToReceiveDataError(String::from("전체 종목을 불러오지 못했습니다.")));
                } else {
                    return Ok(ticker_list);
                }
            }
            Err(e) => Err(e)
        }
    }

    // {
    // "candle_acc_trade_price":106292470.96922,
    // "candle_acc_trade_volume":2.61664687,
    // "candle_date_time_kst":"2023-07-02T23:15:00",
    // "candle_date_time_utc":"2023-07-02T14:15:00",
    // "high_price":40630000.0,
    // "low_price":40588000.0,"market":"KRW-BTC",
    // "opening_price":40625000.0,
    // "timestamp":1688307403765,
    // "trade_price":40625000.0,
    // "unit":5
    // }
    pub async fn get_recent_market_data(&self, ticker: &str, count: u8) -> Result<DataFrame, UPBitError> {
        let url = format!("https://api.upbit.com/v1/candles/minutes/5?market={}&count={}", ticker, count);
        match request_get(&self.reqwest_client, &url, CallMethod::Public).await {
            Ok(json_array) => {

                let mut basic_json = String::new();
                for json in json_array {
                    basic_json.push_str(json.to_string().as_str());
                    basic_json.push_str("\n");
                }
                basic_json.pop();

                let file = std::io::Cursor::new(basic_json);
                let df = JsonReader::new(file)
                    .with_json_format(JsonFormat::JsonLines)
                    .finish()
                    .unwrap()
                    .select([
                        "timestamp",
                        "candle_date_time_kst",
                        "high_price",
                        "low_price",
                        "opening_price",
                        "trade_price",
                    ])
                    .unwrap()
                    .sort(["timestamp"], false)
                    .unwrap();
                Ok(df)
            }
            Err(e) => Err(e)
        }
    }

    pub async fn check_market_trend_percent(&self) -> Result<f64, UPBitError> {

        let btc_df = match self.get_recent_market_data("KRW-BTC", 200).await {
            Ok(df) => df.lazy().select([
                col("trade_price")
            ]).collect().unwrap(),
            Err(e) => return Err(e)
        };
        println!("{}", btc_df);

        let eth_df = match self.get_recent_market_data("KRW-ETH", 200).await {
            Ok(df) => df.lazy().select([
                col("trade_price")
            ]).collect().unwrap(),
            Err(e) => return Err(e)
        };
        println!("{}", eth_df);

        let btc_trend = btc_df.clone().lazy().select([
                col("trade_price")
                    .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                    .alias("ewm_mean")
            ]).collect().unwrap();

        let eth_trend = eth_df.clone().lazy().select([
            col("trade_price")
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("ewm_mean")
        ]).collect().unwrap();

        let btc_ewm = match btc_trend.column("ewm_mean").unwrap().get(btc_trend.column("ewm_mean").unwrap().len()-1).unwrap() {
            AnyValue::Float64(float) => float,
            _ => 0.0
        };
        let btc_last = match btc_df.column("trade_price").unwrap().get(btc_df.column("trade_price").unwrap().len()-1).unwrap() {
            AnyValue::Float64(float) => float,
            _ => 0.0
        };
        let eth_ewm = match eth_trend.column("ewm_mean").unwrap().get(eth_trend.column("ewm_mean").unwrap().len()-1).unwrap() {
            AnyValue::Float64(float) => float,
            _ => 0.0
        };
        let eth_last = match eth_df.column("trade_price").unwrap().get(eth_df.column("trade_price").unwrap().len()-1).unwrap() {
            AnyValue::Float64(float) => float,
            _ => 0.0
        };
        return Ok( 100.0*( 2.0*((btc_last-btc_ewm)/btc_ewm) + ((eth_last-eth_ewm)/eth_ewm) )/3.0);
    }
}
