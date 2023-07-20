use reqwest::{Client, Response, RequestBuilder};
use serde_json::{Value};
use serde::de::DeserializeOwned;
use crate::upbit::{UpbitAccount, response::*};
use tokio::{time};
use tokio::time::Duration;

// UPBit API 측과 상관 없는 에러
#[derive(Debug)]
pub enum InternalRequestError {
    ErrorWhileSend,
    WrongMethod,
}

// UPBit API 응답과 관련한 에러
#[derive(Debug)]
pub enum UpbitResponseError {
    MismatchedResponseType,
    TooManyApiCall,
}

enum RequestMethod {
    Get,
    Post,
}

struct UpbitRequestBuilder {}

impl Default for UpbitRequestBuilder {
    fn default() -> Self {
        UpbitRequestBuilder {}
    }
}

impl UpbitRequestBuilder {
    fn get(self, url: String) -> UpbitRequestConfig {
        UpbitRequestConfig {
            url,
            method: RequestMethod::Get,
            parameters: std::collections::HashMap::new(),
        }
    }

    fn post(self, url: String) -> UpbitRequestConfig {
        UpbitRequestConfig {
            url,
            method: RequestMethod::Post,
            parameters: std::collections::HashMap::new(),
        }
    }
}

struct UpbitRequestConfig {
    url: String,
    method: RequestMethod,
    parameters: std::collections::HashMap<String, String>,
}

impl UpbitRequestConfig {
    fn add_parameter(mut self, key: &str, value: &str) -> Self {
        self.parameters.insert(key.to_string(), value.to_string());
        self
    }

    // GET만 가능
    fn public(mut self) -> Result<UpbitRequest, InternalRequestError> {
        if !self.parameters.is_empty() {
            let (query_string, _) = generate_request_body(&self.parameters);
            self.url.push_str("?");
            self.url.push_str(&query_string);
        }

        let with_method = match self.method {
            RequestMethod::Get => Client::default().get(self.url),
            _ => return Err(InternalRequestError::WrongMethod),
        };

        let upbit_request = UpbitRequest {
            reqwest_builder: with_method
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
        };

        Ok(upbit_request)
    }

    fn private(mut self, upbit_account: &UpbitAccount) -> Result<UpbitRequest, InternalRequestError> {
        use std::collections::BTreeMap;
        use hmac::{Hmac, Mac};
        use jwt::SignWithKey;
        use sha2::{Sha256, Sha512, Digest};

        let (query_string, json_string) = generate_request_body(&self.parameters);
        if !self.parameters.is_empty() {
            self.url.push_str("?");
            self.url.push_str(&query_string);
        }

        let uuid = uuid::Uuid::new_v4().to_string();
        let key: Hmac<Sha256> = Hmac::new_from_slice(upbit_account.secret_key.as_bytes()).unwrap();
        let mut claims: BTreeMap<&str, &str> = BTreeMap::new();
        claims.insert("access_key", &upbit_account.access_key);
        claims.insert("nonce", &uuid);


        let with_method = match self.method {
            RequestMethod::Get => {
                let token_str = claims.sign_with_key(&key).unwrap();
                Client::default()
                    .get(self.url)
                    .bearer_auth(&token_str)
            }
            RequestMethod::Post => {
                let mut buf = [0u8; 1024];
                let query_hash = Sha512::digest(&query_string);
                let hash_string = base16ct::lower::encode_str(&query_hash, &mut buf).expect("JWT 생성을 위한 버퍼 크기가 너무 작습니다.");
                claims.insert("query_hash", &hash_string);
                claims.insert("query_hash_alg", "SHA512");
                let token_str = claims.sign_with_key(&key).unwrap();
                Client::default()
                    .post(self.url)
                    .json(&json_string)
                    .bearer_auth(&token_str)
            }
        };


        let upbit_request = UpbitRequest {
            reqwest_builder: with_method
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")

        };

        Ok(upbit_request)
    }
}

struct UpbitRequest {
    reqwest_builder: RequestBuilder,
}

impl UpbitRequest {
    async fn execute(self) -> Result<UpbitResponse, InternalRequestError> {
        return match self.reqwest_builder.send().await {
            Ok(response) => Ok(UpbitResponse { reqwest_response: response }),
            Err(_) => Err(InternalRequestError::ErrorWhileSend),
        }
    }
}

struct UpbitResponse {
    reqwest_response: Response,
}

impl UpbitResponse {
    async fn response<T>(self) -> Result<T, UpbitResponseError>
        where
    T: DeserializeOwned {
        return match self.reqwest_response.json::<T>().await {
            Ok(deserialized) => Ok(deserialized),
            Err(_) => Err(UpbitResponseError::MismatchedResponseType),
        }
    }
}

// HashMap 형식으로 작성된 body를 입력받아 (쿼리 스트링, JSON) 형식으로 반환합니다.
fn generate_request_body(parameters: &std::collections::HashMap<String, String>) -> (String, String) {
    if parameters.is_empty() {
        return (String::new(), String::new());
    }

    let mut query_string = String::new();
    let mut json_string = String::from("{");
    for (k, v) in parameters {
        let query_part = format!("{}={}&", k, v);
        let json_part = format!("\"{}\": \"{}\", ", k, v);
        query_string.push_str(&query_part);
        json_string.push_str(&json_part);
    }
    query_string.pop().expect("#");
    json_string.pop().expect("#");
    json_string.pop().expect("#");
    json_string.push_str("}");

    (query_string, json_string)
}

pub async fn get_all_balances(account: &UpbitAccount) -> Vec<Balance> {
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    let mut count: u8 = 0;
    loop {
        if count > 20 { panic!("잔고 정보를 불러올 수 없습니다.") }
        count += 1;
        interval.tick().await;
        match UpbitRequestBuilder::default()
            .get("https://api.upbit.com/v1/accounts".to_string())
            .private(account).unwrap()
            .execute().await.unwrap()
            .response::<Vec<Balance>>().await {
            Ok(vec) => return vec,
            Err(_) => {
                match UpbitRequestBuilder::default()
                    .get("https://api.upbit.com/v1/accounts".to_string())
                    .private(account).unwrap()
                    .execute().await.unwrap()
                    .response::<Balance>().await {
                    Ok(one) => return vec![one],
                    Err(_) => continue
                }
            }
        }
    }
}

pub async fn get_balance_of(account: &UpbitAccount, ticker: &str) -> Option<f64> {
    // KRW-XXX의 꼴을 XXX로 만들고, KRW일 경우에는 유지
    let search_for = if ticker == "KRW" {
        "KRW"
    } else {
        ticker.split("-").collect::<Vec<&str>>()[1]
    };

    let balances = get_all_balances(account).await
        .iter()
        .filter(|balance| balance.currency == search_for)
        .map(|balance| balance.balance)
        .collect::<Vec<f64>>();

    if balances.is_empty() {
        None
    } else {
        Some(balances[0])
    }
}

#[allow(dead_code)]
pub async fn get_price_of(ticker: &str) -> Result<f64, UpbitResponseError> {
    match UpbitRequestBuilder::default()
        .get(format!("https://api.upbit.com/v1/ticker?markets={ticker}"))
        .public().unwrap()
        .execute().await.unwrap()
        .response::<Vec<Value>>().await {
        Ok(jsons) => Ok(jsons[0]["trade_price"].as_f64().unwrap()),
        Err(_) => Err(UpbitResponseError::TooManyApiCall),
    }
}

#[allow(dead_code)]
pub async fn guaranteed_get_price_of(ticker: &str) -> f64 {
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    loop {
        interval.tick().await;
        match get_price_of(ticker).await {
            Ok(price) => return price,
            Err(_) => continue,
        }
    }
}

pub async fn buy_market_order(account: &UpbitAccount, ticker: &str, budget: f64) {
    let budget_string = budget.to_string();
    UpbitRequestBuilder::default()
        .post("https://api.upbit.com/v1/orders".to_string())
        .add_parameter("market", ticker)
        .add_parameter("side", "bid")
        .add_parameter("price", &budget_string)
        .add_parameter("ord_type", "price")
        .private(account).unwrap()
        .execute().await.unwrap()
        .response::<Value>().await.unwrap();
}

pub async fn sell_market_order(account: &UpbitAccount, ticker: &str, ratio: f64) -> Result<(), String> {
    if ratio < 0.0 || ratio > 100.0 {
        return Err("판매 비율이 잘못되었습니다.".to_string())
    }

    if let Some(balance) = get_balance_of(account, ticker).await {
        let to_sell = (balance * ratio/100.0).to_string();

        UpbitRequestBuilder::default()
            .post("https://api.upbit.com/v1/orders".to_string())
            .add_parameter("market", ticker)
            .add_parameter("side", "ask")
            .add_parameter("volume", &to_sell)
            .add_parameter("ord_type", "market")
            .private(account).unwrap()
            .execute().await.unwrap()
            .response::<Value>().await.unwrap();

        Ok(())
    } else {
        Err("판매할 보유량이 없습니다.".to_string())
    }
}

pub enum CandleUnit {
    #[allow(unused)]
    Min1,
    #[allow(unused)]
    Min3,
    #[allow(unused)]
    Min5,
    #[allow(unused)]
    Min10,
    #[allow(unused)]
    Min30,
    #[allow(unused)]
    Hour1,
    #[allow(unused)]
    Hour4,
}

pub async fn get_candle_data(ticker: &str, unit: &CandleUnit, count: u8) -> Result<Vec<CandleData>, UpbitResponseError> {
    let interval_url = match unit {
        CandleUnit::Min1 => "minutes/1",
        CandleUnit::Min3 => "minutes/3",
        CandleUnit::Min5 => "minutes/5",
        CandleUnit::Min10 => "minutes/10",
        CandleUnit::Min30 => "minutes/30",
        CandleUnit::Hour1 => "minutes/60",
        CandleUnit::Hour4 => "minutes/240",
    };

    UpbitRequestBuilder::default()
        .get(format!("https://api.upbit.com/v1/candles/{interval_url}?market={ticker}&count={count}"))
        .public().unwrap()
        .execute().await.unwrap()
        .response::<Vec<CandleData>>().await.map_err(|_| UpbitResponseError::TooManyApiCall)
}

pub async fn guaranteed_get_candle_data(ticker: &str, unit: CandleUnit, count: u8) -> Vec<CandleData> {
    let mut interval = time::interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        match get_candle_data(ticker, &unit, count).await {
            Ok(datas) => return datas,
            Err(_) => continue,
        }
    }
}

pub async fn get_all_tickers() -> Vec<String> {
    let response = UpbitRequestBuilder::default()
        .get("https://api.upbit.com/v1/market/all".to_string())
        .public().unwrap()
        .execute().await.unwrap()
        .response::<Vec<Ticker>>().await.unwrap();

    let only_krws = response
        .into_iter()
        .filter(|ticker| ticker.market.contains("KRW"))
        .map(|ticker| ticker.market)
        .collect::<Vec<String>>();

    only_krws
}

// // 구매 예산이 너무 적거나, 이미 많이 보유하고 있으면 구매하지 않음
// pub async fn buy(&self, public_upbit: &UPBitSocket, ticker: &str) -> BuyResult {
//     // 현재 모든 자산을 KRW로 환산해서 저장
//     let mut krw = 0.0;
//     let my_balances = match self.get_all_balances().await {
//         Ok(map) => map,
//         Err(e) => {
//             // 자산이 없는 경우만 존재
//             eprintln!("{}", e);
//             return BuyResult::NotEnoughBudget;
//         }
//     };
//     for (t, b) in my_balances {
//         if t == "KRW" {
//             krw += b;
//         } else {
//             let p = public_upbit.get_price_of(&t).await.unwrap();
//             krw += b * p;
//         }
//     }
//
//     // 현재 종목 가격
//     let price = public_upbit.get_price_of(ticker).await.unwrap();
//
//     // 한 종목에 구매가 몰리는 것을 방지하기 위한 기준값
//     let limit_percent =  45.0; // %
//
//     // 구매 예산을 전체 자산의 n%로 측정
//     let n_percent = 40.0; // %
//     let budget = krw * n_percent/100.0;
//
//     // 책정 예산이 기준 금액보다 적으면 구매 취소
//     let budget_minimum = 10000.0;
//     if budget < budget_minimum { return BuyResult::NotEnoughBudget; }
//
//     // 현재 종목에 대해 내가 보유한 양
//     let already_bought = price * self.get_balance_of(ticker).await.unwrap();// KRW
//
//     // 이번 구매로 인해 기준값을 넘게 된다면 구매 취소
//     if already_bought + budget > krw*limit_percent/100.0 { return BuyResult::Overbalance; }
//
//     // 구매
//     return match self.buy_market_order(ticker, budget).await{
//         Ok(()) => BuyResult::Bought,
//         Err(UPBitError::FailedToTradeError(_)) => BuyResult::NotEnoughBudget, // 그냥 단순히 보유금 없음
//         Err(e) => panic!("{}", e),
//     }
// }
//
// // 해당 종목을 전체 판매
// async fn sell(&self, ticker: &str) {
//     match self.sell_market_order_by_percent(ticker, 100.0).await {
//         Ok(_) => (),
//         Err(UPBitError::FailedToTradeError(detail)) => eprintln!("{}", detail),
//         Err(e) => panic!("{}", e)
//     }
// }
//
// // 현재 보유하고 있는 종목을 판매해야 할 지 판단
// pub async fn sell_decision(&mut self, public_upbit: &UPBitSocket) {
//     for (ticker, coin) in &self.coins {
//         // RSI가 기준치 이상으로 올라갔다가 내려가면 판매
//         let rsi_limit = 65.0;
//         let never_sell_bottom = 55.0;
//         let previous_rsi = public_upbit.get_rsi(ticker, coin.get_dataframe(), 1).await.unwrap();
//         let current_rsi = public_upbit.get_rsi(ticker, coin.get_dataframe(), 0).await.unwrap();
//
//         if self.max_rsis.contains_key(ticker) {
//             if self.max_rsis.get(ticker).unwrap() < &current_rsi {
//                 self.max_rsis.insert(ticker.to_string(), current_rsi);
//             }
//         } else {
//             self.max_rsis.insert(ticker.to_string(), current_rsi);
//         }
//
//         if self.max_rsis.get(ticker).unwrap() > &rsi_limit
//             && (current_rsi < rsi_limit || current_rsi > 75.0)
//         {
//             self.sell(ticker).await;
//         }
//     }
// }
//
// pub enum UPBitError {
//     DuplicatedNonceError,
//     InvalidRequestError,
//     FailedToReceiveDataError(String),
//     URLError,
//     InvalidJsonError,
//     OtherError(String),
//     FailedToTradeError(String),
//     TooManyRequestError,
// }
//
// impl std::fmt::Debug for UPBitError {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         match self {
//             UPBitError::DuplicatedNonceError => write!(f, "요청에 중복된 UUID가 사용되었습니다."),
//             UPBitError::InvalidRequestError => write!(f, "잘못된 요청입니다."),
//             UPBitError::FailedToReceiveDataError(detail) => write!(f, "데이터를 받아오지 못했습니다: {}", detail),
//             UPBitError::URLError => write!(f, "요청한 URL로 요청을 보낼 수 없습니다."),
//             UPBitError::InvalidJsonError => write!(f, "응답 데이터를 JSON 형식으로 변환할 수 없습니다."),
//             UPBitError::OtherError(detail) => write!(f, "오류가 발생했습니다: {}", detail),
//             UPBitError::FailedToTradeError(detail) => write!(f, "거래에 실패했습니다: {}", detail),
//             UPBitError::TooManyRequestError => write!(f, "UPBit API 호출 횟수를 초과했습니다."),
//         }
//     }
// }
//
// impl std::fmt::Display for UPBitError {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         match self {
//             UPBitError::DuplicatedNonceError => write!(f, "요청에 중복된 UUID가 사용되었습니다."),
//             UPBitError::InvalidRequestError => write!(f, "잘못된 요청입니다."),
//             UPBitError::FailedToReceiveDataError(detail) => write!(f, "데이터를 받아오지 못했습니다: {}", detail),
//             UPBitError::URLError => write!(f, "요청한 URL로 요청을 보낼 수 없습니다."),
//             UPBitError::InvalidJsonError => write!(f, "응답 데이터를 JSON 형식으로 변환할 수 없습니다."),
//             UPBitError::OtherError(detail) => write!(f, "오류가 발생했습니다: {}", detail),
//             UPBitError::FailedToTradeError(detail) => write!(f, "거래에 실패했습니다: {}", detail),
//             UPBitError::TooManyRequestError => write!(f, "UPBit API 호출 횟수를 초과했습니다."),
//         }
//     }
// }
//
// async fn request_get(reqwest_client: &reqwest::Client, url: &str, method: CallMethod<'_>) -> Result<Vec<serde_json::Value>, UPBitError> {
//     use std::collections::BTreeMap;
//     use hmac::{Hmac, Mac};
//     use sha2::Sha256;
//     use jwt::SignWithKey;
//
//     let response = match method {
//         CallMethod::Public => {
//             reqwest_client.get(url)
//                 .header("Content-Type", "application/json")
//                 .send().await
//         }
//         CallMethod::Private((access_key, secret_key)) => {
//             let uuid = uuid::Uuid::new_v4().to_string();
//             let key: Hmac<Sha256> = Hmac::new_from_slice(secret_key.as_bytes()).unwrap();
//             let mut claims = BTreeMap::new();
//             claims.insert("access_key", access_key);
//             claims.insert("nonce", &uuid);
//             let token_str = claims.sign_with_key(&key).unwrap();
//
//             reqwest_client.get(url)
//                 .header("Accept", "application/json")
//                 .header("Content-Type", "application/json")
//                 .bearer_auth(&token_str)
//                 .send().await
//         }
//     };
//
//     return if let Ok(res) = response {
//         response_to_json(res).await
//     } else {
//         Err(UPBitError::InvalidJsonError)
//     };
// }
//
// async fn request_post(reqwest_client: &reqwest::Client, url: &str, body: &std::collections::HashMap<&str, &str>, method: CallMethod<'_>) -> Result<Vec<serde_json::Value>, UPBitError> {
//     use serde_json::json;
//     use std::collections::BTreeMap;
//     use hmac::{Hmac, Mac};
//     use jwt::SignWithKey;
//     use sha2::{Sha256, Sha512, Digest};
//
//     let response = match method {
//         CallMethod::Public => {
//             return Err(UPBitError::OtherError(String::from("POST 요청은 Public 메소드로 불가능합니다.")))
//         }
//         CallMethod::Private((access_key, secret_key)) => {
//             let (query_string, json_string) = generate_request_body(&body);
//             let mut buf = [0u8; 1024];
//             let query_hash = Sha512::digest(&query_string);
//             let hash_string = match base16ct::lower::encode_str(&query_hash, &mut buf) {
//                 Ok(value) => String::from(value),
//                 Err(_) => {
//                     let mut buf = [0u8; 4096];
//                     let query_hash = Sha512::digest(&query_string);
//
//                     match base16ct::lower::encode_str(&query_hash, &mut buf) {
//                         Ok(v) => String::from(v),
//                         Err(_) => return Err(UPBitError::OtherError(String::from("요청 body의 크기에 비해 이를 해시하기 위한 버퍼의 크기가 너무 작습니다.")))
//                     }
//                 }
//             };
//
//             let uuid = uuid::Uuid::new_v4().to_string();
//             let key: Hmac<Sha256> = Hmac::new_from_slice(secret_key.as_bytes()).unwrap();
//             let mut claims = BTreeMap::new();
//             claims.insert("access_key", access_key);
//             claims.insert("nonce", &uuid);
//             claims.insert("query_hash", &hash_string);
//             claims.insert("query_hash_alg", "SHA512");
//             let token_str = claims.sign_with_key(&key).unwrap();
//
//             reqwest_client.post(url)
//                 .header("Accept", "application/json")
//                 .header("Content-Type", "application/json")
//                 .body(json_string)
//                 .bearer_auth(&token_str)
//                 .send().await
//         }
//     };
//
//     return if let Ok(res) = response {
//         response_to_json(res).await
//     } else {
//         Err(UPBitError::URLError)
//     };
// }
//
// async fn response_to_json(response: reqwest::Response) -> Result<Vec<serde_json::Value>, UPBitError> {
//     // JSON 형식인지 판별
//     let json = match response.json::<serde_json::Value>().await {
//         Ok(j) => j,
//         Err(_) => {
//             return Err(UPBitError::TooManyRequestError);
//         }
//     };
//
//     // to_string() 값이 2 ( {} )면 요청 에러 반환
//     if json.to_string().len() == 2 {
//         return Err(UPBitError::InvalidRequestError);
//     }
//
//     // 배열 형태면 그대로 반환, 배열이 아니면 배열로 반환
//     return if json.is_array() {
//         Ok(json.as_array().unwrap().clone())
//     } else {
//         Ok(vec![json])
//     }
// }
