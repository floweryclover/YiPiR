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
}
