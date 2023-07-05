use polars::series::ops::NullBehavior;
use polars::prelude::*;
use crate::upbit::{UPBitSocket, UPBitError, CallMethod, request_get, request_post, generate_request_body, response_to_json};

impl UPBitSocket {
    pub async fn get_tickers_sortby_volume(&self) -> Result<Vec<String>, UPBitError> {
        // API 호출 한계 도달시 다시 호출
        let tickers = match self.get_all_available_tickers().await {
            Ok(tckrs) => tckrs,
            Err(e) => panic!("{}", e)
        };

        let mut url = String::from("https://api.upbit.com/v1/ticker?markets=");
        for ticker in tickers {
            url.push_str(&ticker);
            url.push_str(",");
        }
        url.pop();

        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            match request_get(&self.reqwest_client, &url, CallMethod::Public).await {
                Ok(json_array) => {
                    let mut vec = Vec::new();
                    for item in json_array {
                        vec.push((String::from(item["market"].as_str().unwrap()), item["trade_price"].as_f64().unwrap() * item["acc_trade_volume_24h"].as_f64().unwrap()));
                    }
                    vec.sort_by(|(_, va), (_, vb)| vb.partial_cmp(va).unwrap());
                    let mut return_vec = Vec::new();
                    for (ticker, _) in vec {
                        return_vec.push(ticker);
                    }
                    return Ok(return_vec);
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    pub async fn get_all_available_tickers(&self) -> Result<Vec<String>, UPBitError> {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            match request_get(&self.reqwest_client, "https://api.upbit.com/v1/market/all", CallMethod::Public).await {
                Ok(json_array) => {
                    let mut ticker_list: Vec<String> = Vec::new();//json_array.iter().filter(|j| j["market"].as_str().unwrap().contains("KRW")).map(|j| String::from(j["market"].as_str().unwrap())).collect();
                    for json in json_array {
                        if json["market"].as_str().unwrap().contains("KRW") {//&& json["market_warning"].as_str().unwrap() != "CAUTION" {
                            ticker_list.push(String::from(json["market"].as_str().unwrap()));
                        }
                    }
                    return if ticker_list.is_empty() {
                        Err(UPBitError::FailedToReceiveDataError(String::from("전체 종목을 불러오지 못했습니다.")))
                    } else {
                        Ok(ticker_list)
                    }
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }
    
    pub async fn get_recent_market_data(&self, ticker: &str, count: u8) -> Result<DataFrame, UPBitError> {
        let url = format!("https://api.upbit.com/v1/candles/minutes/5?market={}&count={}", ticker, count);
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
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
                    return Ok(df);
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    // 종목의 현재가를 KRW로 얻어오기
    pub async fn get_price_of(&self, ticker: &str) -> Result<f64, UPBitError> {
        // 현재 실시간 데이터에 키가 존재하면 거기서 값을 가져옴
        {
            let mutex = self.realtime_price.lock().unwrap();
            if (*mutex).contains_key(ticker) {
                return Ok((*mutex).get(ticker).unwrap().clone());
            }
        }

        // 실시간 데이터에서 찾지 못했으므로, UPBit API에서 새로 받아옴
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        let url = format!("https://api.upbit.com/v1/ticker?markets={}", ticker);
        loop {
            interval.tick().await;
            match request_get(&self.reqwest_client, &url, CallMethod::Public).await {
                Ok(json_array) => {
                    let json = &json_array[0]; // 한 개의 ticker만 조회하니 무조건 0번 인덱스만 존재
                    match json["trade_price"].as_f64() {
                        Some(f) => return Ok(f),
                        None => panic!("Ticker:{},  {}", ticker, json.to_string()),
                    }
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    // bei: BTC와 ETH로 측정한 시장 가격의 저점/고점 상황 (배율: x100) - 낮을수록 구매 강도 상승.
    // delta: 변화량. 음수에서 양수로 변환되는 시점이 구매 강도 상승.
    // rsi: BERSI
    // previous = 0 현재, ex) previous = 1: 1x5분전 기준
    pub async fn btc_eth_indicator(&self, previous: u8) -> Result<(f64, f64, f64), UPBitError> {
        let btc_df = match self.get_recent_market_data("KRW-BTC", 200).await {
            Ok(df) => {
                df.lazy().select([
                    col("trade_price").alias("btc_price"),
                ])
                    .collect().unwrap()
                    .with_row_count("no", None).unwrap()
            }
            Err(e) => return Err(e)
        };

        let eth_df = match self.get_recent_market_data("KRW-ETH", 200).await {
            Ok(df) => {
                df.lazy().select([
                    col("trade_price").alias("eth_price")
                ])
                    .collect().unwrap()
                    .with_row_count("no", None).unwrap()
            }
            Err(e) => return Err(e)
        };

        let indicator_df = btc_df.join(&eth_df, ["no"], ["no"], JoinType::Inner, None).unwrap()
            .lazy().select([
            col("btc_price"),
            col("btc_price").ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14)).alias("btc_ewm"),
            col("btc_price").diff(1, NullBehavior::Ignore).alias("btc_diff"),
            col("eth_price"),
            col("eth_price").ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14)).alias("eth_ewm"),
            col("eth_price").diff(1, NullBehavior::Ignore).alias("eth_diff"),
        ])
            .collect().unwrap()
            .lazy().select([
            ((lit(2.0)*(col("btc_price")-col("btc_ewm"))/col("btc_ewm") + lit(1.0)*(col("eth_price")-col("eth_price"))/col("eth_ewm")) /lit(3.0)).alias("bei"),
            when(col("btc_diff").lt(lit(0)))
                .then(lit(0))
                .otherwise(col("btc_diff"))
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("btc_au"),
            when(col("btc_diff").gt(lit(0)))
                .then(lit(0))
                .otherwise(col("btc_diff"))
                .abs()
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("btc_ad"),
            when(col("eth_diff").lt(lit(0)))
                .then(lit(0))
                .otherwise(col("eth_diff"))
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("eth_au"),
            when(col("eth_diff").gt(lit(0)))
                .then(lit(0))
                .otherwise(col("eth_diff"))
                .abs()
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("eth_ad"),
        ])
            .collect().unwrap()
            .lazy().select([
            (col("bei")*lit(100.0)).round(3),
            (col("bei").diff(1, NullBehavior::Ignore)*lit(100.0)).round(3).alias("delta"),
            (((lit(2.0)*(lit(100.0) - (lit(100.0) / (lit(1.0)+(col("btc_au")/col("btc_ad")) )))) + (lit(1.0)*(lit(100.0) - (lit(100.0) / (lit(1.0)+(col("eth_au")/col("eth_ad")) )))))/lit(3.0)).alias("bersi")
        ])
            .collect().unwrap()
            .drop_nulls::<String>(None).unwrap();

        let bei = match indicator_df.column("bei").unwrap().get(indicator_df.column("bei").unwrap().len()-1-(previous as usize)).unwrap() {
            AnyValue::Float64(f) => f,
            _ => return Err(UPBitError::OtherError(String::from("BEI 측정 후 실수 변환에 실패했습니다.")))
        };
        let delta = match indicator_df.column("delta").unwrap().get(indicator_df.column("delta").unwrap().len()-1-(previous as usize)).unwrap() {
            AnyValue::Float64(f) => f,
            _ => return Err(UPBitError::OtherError(String::from("BEI 측정 후 실수 변환에 실패했습니다.")))
        };
        let bersi = match indicator_df.column("bersi").unwrap().get(indicator_df.column("bersi").unwrap().len()-1-(previous as usize)).unwrap() {
            AnyValue::Float64(f) => f,
            _ => return Err(UPBitError::OtherError(String::from("BEI 측정 후 실수 변환에 실패했습니다.")))
        };
        return Ok((bei, delta, bersi));
    }
}
