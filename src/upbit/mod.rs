use actix_web::web;
use polars::frame::DataFrame;
use serde_json::json;
use std::sync::{Arc, Mutex};

pub mod restful;
pub mod public;
pub mod coin;

enum CallMethod<'a> {
    Public,
    Private((&'a str, &'a str)),
}

pub struct UPBitSocket {
    recommended_coins: std::collections::HashMap<String, coin::Coin>,
    reqwest_client: reqwest::Client,
    realtime_price: Arc<Mutex<std::collections::HashMap<String, f64>>>,
    previous_bei_delta: f64,
    previous_bersi: f64,
}

pub struct UPBitService {}

impl UPBitService {
    pub fn new() -> Self {
        UPBitService {}
    }

    pub async fn run(&self) -> tokio::task::JoinHandle<()> {
        use tokio::{task, time};
        use tokio::time::Duration;
        use tungstenite::{connect, Message};

        // 메인 루프
        task::spawn(async move {
            // use polars::prelude::*;
            use crate::upbit::restful::UPBitAccount;
            let upbit_floweryclover = UPBitAccount::new("qynTp0pDQGLkYl4VmoZax9ftGjQNe5YwLpJ7X4fm", "0ikxDGSb4XkwndGIYhd1yNzE0jUji1lQHqEPxWfx");
            let mut upbit_socket = UPBitSocket::new();
            // 웹소켓 수신
            let (mut stream, response) = connect("wss://api.upbit.com/websocket/v1").unwrap();

            let tickers = upbit_socket.get_tickers_sortby_volume().await.unwrap();

            let uuid = uuid::Uuid::new_v4().to_string();
            let mut jsoned_tickers = Vec::new();
            for ticker in tickers {
                jsoned_tickers.push(json!(ticker));
            }

            let send_json = json!([
                {"ticket": uuid},
                {
                    "type": "ticker",
                    "codes": serde_json::Value::Array(jsoned_tickers),
                    "isOnlyRealtime": true
                },
                {"format": "DEFAULT"}
            ]).to_string();

            stream.write_message(Message::Text(send_json)).unwrap();

            let realtime_price_arc = upbit_socket.realtime_price.clone();
            task::spawn_blocking(move || {
                loop {
                    if let Message::Binary(ref binary) = stream.read_message().unwrap() {
                        let json: serde_json::Value = serde_json::from_slice(binary).unwrap();
                        {
                            let mut realtime_price = realtime_price_arc.lock().unwrap();
                            (*realtime_price).insert(String::from(json["code"].as_str().unwrap()), json["trade_price"].as_f64().unwrap());
                        }
                    }
                }
            });

            let mut count: u16 = 300;
            let mut interval = time::interval(Duration::from_secs(1));
            let (mut bei, mut delta, mut bersi) = (1.0, 1.0, 50.0);
                loop {

                if count == 300 {
                    (bei, delta, bersi) = upbit_socket.refresh_recommended_coins().await.unwrap();
                    println!("{} {} {}", bei, delta, bersi);
                    count = 1;
                }
                if !upbit_socket.recommended_coins.is_empty() {
                    for (ticker, coin) in &mut upbit_socket.recommended_coins {
                        if coin.buy_decision(&upbit_socket.realtime_price).await {

                        }
                    }
                }
                interval.tick().await;
                count += 1;
            }
        })
    }
}

impl UPBitSocket {
    pub fn new() -> Self {
        use crate::upbit::{UPBitSocket};

        UPBitSocket {
            recommended_coins: std::collections::HashMap::new(),
            reqwest_client: reqwest::Client::new(),
            realtime_price: Arc::new(Mutex::new(std::collections::HashMap::new())),
            previous_bei_delta: 0.0,
            previous_bersi: 0.0,
        }
    }
}

pub enum UPBitError {
    DuplicatedNonceError,
    InvalidParameterError,
    FailedToReceiveDataError(String),
    RequestError,
    InvalidJsonError,
    OtherError(String),
    FailedToTradeError(String),
    TooManyRequestError,
}

impl std::fmt::Debug for UPBitError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            UPBitError::DuplicatedNonceError => write!(f, "요청에 중복된 UUID가 사용되었습니다."),
            UPBitError::InvalidParameterError => write!(f, "요청에 잘못된 파라미터가 존재합니다."),
            UPBitError::FailedToReceiveDataError(detail) => write!(f, "데이터를 받아오지 못했습니다: {}", detail),
            UPBitError::RequestError => write!(f, "요청한 URL로 요청을 보낼 수 없습니다."),
            UPBitError::InvalidJsonError => write!(f, "응답 데이터를 JSON 형식으로 변환할 수 없습니다."),
            UPBitError::OtherError(detail) => write!(f, "오류가 발생했습니다: {}", detail),
            UPBitError::FailedToTradeError(detail) => write!(f, "거래에 실패했습니다: {}", detail),
            UPBitError::TooManyRequestError => write!(f, "UPBit API 호출 횟수를 초과했습니다."),
        }
    }
}

impl std::fmt::Display for UPBitError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            UPBitError::DuplicatedNonceError => write!(f, "요청에 중복된 UUID가 사용되었습니다."),
            UPBitError::InvalidParameterError => write!(f, "요청에 잘못된 파라미터가 존재합니다."),
            UPBitError::FailedToReceiveDataError(detail) => write!(f, "데이터를 받아오지 못했습니다: {}", detail),
            UPBitError::RequestError => write!(f, "요청한 URL로 요청을 보낼 수 없습니다."),
            UPBitError::InvalidJsonError => write!(f, "응답 데이터를 JSON 형식으로 변환할 수 없습니다."),
            UPBitError::OtherError(detail) => write!(f, "오류가 발생했습니다: {}", detail),
            UPBitError::FailedToTradeError(detail) => write!(f, "거래에 실패했습니다: {}", detail),
            UPBitError::TooManyRequestError => write!(f, "UPBit API 호출 횟수를 초과했습니다."),
        }
    }
}

async fn request_get(reqwest_client: &reqwest::Client, url: &str, method: CallMethod<'_>) -> Result<Vec<serde_json::Value>, UPBitError> {
    use std::collections::BTreeMap;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use jwt::SignWithKey;

    let response = match method {
        CallMethod::Public => {
            reqwest_client.get(url)
                .header("Content-Type", "application/json")
                .send().await
        }
        CallMethod::Private((access_key, secret_key)) => {
            let uuid = uuid::Uuid::new_v4().to_string();
            let key: Hmac<Sha256> = Hmac::new_from_slice(secret_key.as_bytes()).unwrap();
            let mut claims = BTreeMap::new();
            claims.insert("access_key", access_key);
            claims.insert("nonce", &uuid);
            let token_str = claims.sign_with_key(&key).unwrap();

            reqwest_client.get(url)
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .bearer_auth(&token_str)
                .send().await
        }
    };

    return if let Ok(res) = response {
        response_to_json(res).await
    } else {
        Err(UPBitError::RequestError)
    };
}

async fn request_post(reqwest_client: &reqwest::Client, url: &str, body: &std::collections::HashMap<&str, &str>, method: CallMethod<'_>) -> Result<Vec<serde_json::Value>, UPBitError> {
    use serde_json::json;
    use std::collections::BTreeMap;
    use hmac::{Hmac, Mac};
    use jwt::SignWithKey;
    use sha2::{Sha256, Sha512, Digest};

    let response = match method {
        CallMethod::Public => {
            return Err(UPBitError::OtherError(String::from("POST 요청은 Public 메소드로 불가능합니다.")))
        }
        CallMethod::Private((access_key, secret_key)) => {
            let (query_string, json_string) = generate_request_body(&body);
            let mut buf = [0u8; 1024];
            let query_hash = Sha512::digest(&query_string);
            let hash_string = match base16ct::lower::encode_str(&query_hash, &mut buf) {
                Ok(value) => String::from(value),
                Err(_) => {
                    let mut buf = [0u8; 4096];
                    let query_hash = Sha512::digest(&query_string);

                    match base16ct::lower::encode_str(&query_hash, &mut buf) {
                        Ok(v) => String::from(v),
                        Err(_) => return Err(UPBitError::OtherError(String::from("요청 body의 크기에 비해 이를 해시하기 위한 버퍼의 크기가 너무 작습니다.")))
                    }
                }
            };

            let uuid = uuid::Uuid::new_v4().to_string();
            let key: Hmac<Sha256> = Hmac::new_from_slice(secret_key.as_bytes()).unwrap();
            let mut claims = BTreeMap::new();
            claims.insert("access_key", access_key);
            claims.insert("nonce", &uuid);
            claims.insert("query_hash", &hash_string);
            claims.insert("query_hash_alg", "SHA512");
            let token_str = claims.sign_with_key(&key).unwrap();

            reqwest_client.post(url)
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .body(json_string)
                .bearer_auth(&token_str)
                .send().await
        }
    };

    return if let Ok(res) = response {
        response_to_json(res).await
    } else {
        Err(UPBitError::RequestError)
    };
}

async fn response_to_json(response: reqwest::Response) -> Result<Vec<serde_json::Value>, UPBitError> {
    // JSON 형식인지 판별
    let json = match response.json::<serde_json::Value>().await {
        Ok(j) => j,
        Err(_) => {
            return Err(UPBitError::TooManyRequestError);
        }
    };

    // 배열 형태면 그대로 반환, 배열이 아니면 배열로 반환
    return if json.is_array() {
        Ok(json.as_array().unwrap().clone())
    } else {
        Ok(vec![json])
    }
}

// HashMap 형식으로 작성된 body를 입력받아 (쿼리 스트링, JSON) 형식으로 반환합니다.
fn generate_request_body(body: &std::collections::HashMap<&str, &str>) -> (String, String) {
    if body.is_empty() {
        return (String::new(), String::new());
    }

    let mut query_string = String::new();
    let mut json_string = String::from("{");
    for (k, v) in body {
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