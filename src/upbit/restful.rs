use crate::upbit::{UPBitError, request_post, request_get, generate_request_body, response_to_json, CallMethod, UPBitSocket};
use crate::upbit::coin::Coin;

pub struct UPBitAccount {
    access_key: String,
    secret_key: String,
    reqwest_client: reqwest::Client,
    my_coins: std::collections::HashMap<String, Coin>,
}

impl UPBitAccount {
    pub fn new(access_key: &str, secret_key: &str) -> UPBitAccount {
        UPBitAccount {
            access_key: String::from(access_key),
            secret_key: String::from(secret_key),
            reqwest_client: reqwest::Client::new(),
            my_coins: std::collections::HashMap::new(),
        }
    }
    pub async fn get_all_balances(&self) -> Result<std::collections::HashMap<String, f64>, UPBitError> {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            match request_get(&self.reqwest_client, "https://api.upbit.com/v1/accounts", CallMethod::Private((&self.access_key, &self.secret_key))).await {
                Ok(json_array) => {
                    let mut return_map = std::collections::HashMap::new();
                    for item in json_array {
                        let mut ticker = String::new();
                        if item["currency"].as_str().unwrap() != "KRW" {
                            ticker = String::from(format!("KRW-{}", item["currency"].as_str().unwrap()));
                        } else {
                            ticker = String::from(item["currency"].as_str().unwrap());
                        }
                        return_map.insert(ticker, item["balance"].as_str().unwrap().parse::<f64>().unwrap());
                    }
                    if return_map.is_empty() {
                        return Err(UPBitError::FailedToReceiveDataError(String::from("현재 보유중인 자산이 없습니다.")));
                    } else {
                        return Ok(return_map);
                    }
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    pub async fn get_balance_of(&self, ticker: &str) -> Result<f64, UPBitError> {
        let search_for = if ticker == "KRW" {
            "KRW"
        } else {
            ticker.split("-").collect::<Vec<&str>>()[1]
        };
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            match request_get(&self.reqwest_client, "https://api.upbit.com/v1/accounts", CallMethod::Private((&self.access_key, &self.secret_key))).await {
                Ok(json_array) => {
                    for item in json_array {
                        if item["currency"] == search_for {
                            return Ok(
                                item["balance"]
                                    .as_str()
                                    .expect("#")
                                    .parse::<f64>()
                                    .expect("#")
                            );
                        }
                    }
                    return Ok(0.0);
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    pub async fn buy_market_order(&self, ticker: &str, price: f64) -> Result<(), UPBitError> {
        let price_str = price.to_string();
        let mut body = std::collections::HashMap::new();
        body.insert("market", ticker);
        body.insert("side", "bid");
        body.insert("price", &price_str);
        body.insert("ord_type", "price");

        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            match request_post(&self.reqwest_client, "https://api.upbit.com/v1/orders", &body, CallMethod::Private((&self.access_key, &self.secret_key))).await {
                Ok(json_array) => {
                    let json = &json_array[0];
                    return match json.get("uuid") {
                        Some(_) => {
                            Ok(())
                        }
                        None => {
                            Err(UPBitError::FailedToTradeError(json["error"]["name"].to_string()))
                        }
                    }
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    pub async fn sell_market_order_by_volume(&self, ticker: &str, volume: f64) -> Result<(), UPBitError> {
        self.sell_market_order(ticker, volume).await
    }

    pub async fn sell_market_order_by_percent(&self, ticker: &str, percent: f64) -> Result<(), UPBitError> {
        if percent < 0.0 || percent > 100.0 {
            return Err(UPBitError::OtherError(String::from(format!("유효하지 않은 백분율입니다: {}", percent.to_string()))));
        }

        let volume = match self.get_balance_of(ticker).await {
            Ok(value) => value,
            Err(e) => return Err(e)
        };

        let volume_str = (volume * (percent/100.0) ).to_string();

        let mut body = std::collections::HashMap::new();
        body.insert("market", ticker);
        body.insert("side", "ask");
        body.insert("volume", &volume_str);
        body.insert("ord_type", "market");

        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            match request_post(&self.reqwest_client, "https://api.upbit.com/v1/orders", &body, CallMethod::Private((&self.access_key, &self.secret_key))).await {
                Ok(json_array) => {
                    let json = &json_array[0];
                    return match json.get("uuid") {
                        Some(_) => {
                            Ok(())
                        }
                        None => {
                            Err(UPBitError::FailedToTradeError(json["error"]["name"].to_string()))
                        }
                    }
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    async fn sell_market_order(&self, ticker: &str, volume: f64) -> Result<(), UPBitError> {
        let volume_str = volume.to_string();
        let mut body = std::collections::HashMap::new();
        body.insert("market", ticker);
        body.insert("side", "ask");
        body.insert("volume", &volume_str);
        body.insert("ord_type", "market");

        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            match request_post(&self.reqwest_client, "https://api.upbit.com/v1/orders", &body, CallMethod::Private((&self.access_key, &self.secret_key))).await {
                Ok(json_array) => {
                    let json = &json_array[0];
                    return match json.get("uuid") {
                        Some(_) => {
                            Ok(())
                        }
                        None => {
                            Err(UPBitError::FailedToTradeError(json["error"]["name"].to_string()))
                        }
                    }
                }
                Err(UPBitError::TooManyRequestError) => continue,
                Err(e) => panic!("{}", e)
            }
        }
    }

    // 구매 예산이 너무 적거나, 이미 많이 보유하고 있으면 구매하지 않음
    pub async fn buy(&self, public_upbit: &UPBitSocket, ticker: &str) {
        // 현재 모든 자산을 KRW로 환산해서 저장
        let mut krw = 0.0;
        let my_balances = match self.get_all_balances().await {
            Ok(map) => map,
            Err(e) => {
                // 자산이 없는 경우만 존재
                eprintln!("{}", e);
                return;
            }
        };
        for (t, b) in my_balances {
            if ticker == "KRW" {
                krw += b;
            } else {
                krw += public_upbit.get_price_of(&t).await.unwrap();
            }
        }

        // 현재 종목 가격
        let price = public_upbit.get_price_of(ticker).await.unwrap();

        // 한 종목에 구매가 몰리는 것을 방지하기 위한 기준값
        let limit_percent =  15.0; // %

        // 구매 예산을 전체 자산의 n%로 측정
        let n_percent = 10.0; // %
        let budget = krw * n_percent/100.0;

        // 책정 예산이 기준 금액보다 적으면 구매 취소
        let budget_minimum = 6000.0;
        if budget < budget_minimum { return; }

        // 현재 종목에 대해 내가 보유한 양
        let already_bought = price * self.get_balance_of(ticker).await.unwrap();// KRW

        // 이번 구매로 인해 기준값을 넘게 된다면 구매 취소
        if already_bought + budget > krw*limit_percent/100.0 { return; }

        // 구매
        self.buy_market_order(ticker, budget).await.unwrap();
    }

    // 해당 종목을 전체 판매
    async fn sell(&self, ticker: &str) {
        self.sell_market_order_by_percent(ticker, 100.0).await.unwrap();
    }

    // 현재 보유하고 있는 종목을 판매해야 할 지 판단
    pub async fn sell_decision(&self, public_upbit: &UPBitSocket,) {
        for (ticker, coin) in &self.my_coins {
            // RSI가 너무 높으면 판매
            let rsi_limit = 60.0;
            let current_rsi = coin.get_rsi(&public_upbit.realtime_price, 0).await.unwrap();
            if current_rsi > rsi_limit {
                self.sell(ticker);
            }
        }
    }
}