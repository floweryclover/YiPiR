use crate::upbit::{UPBitError, UPBitSocket};

impl UPBitSocket {
    //TODO: 5분마다 추천 종목의 정보를 업데이트하는 대신, 아예 5분마다 그냥 추천 종목 자체를 새로 갱신한다면? -> API 호출량이 증가한다는 문제가 있으나 LOOP를 하나만 작성 가능
    // pub async fn ......
    //pub fn run_buy_loop()

    pub fn run_realtime_price_loop(&self, tickers: &Vec<String>) {
        use tungstenite::{connect, Message};
        let (mut stream, response) = connect("wss://api.upbit.com/websocket/v1").unwrap();

        let uuid = uuid::Uuid::new_v4().to_string();
        let mut jsoned_tickers = Vec::new();
        for ticker in tickers {
            jsoned_tickers.push(serde_json::json!(ticker));
        }

        let send_json = serde_json::json!([
        {"ticket": uuid},
        {
            "type": "ticker",
            "codes": serde_json::Value::Array(jsoned_tickers),
            "isOnlyRealtime": true
        },
        {"format": "DEFAULT"}
    ]).to_string();

        stream.write_message(Message::Text(send_json)).unwrap();

        let arc_cloned = self.realtime_price.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                if let Message::Binary(ref binary) = stream.read_message().unwrap() {
                    let json: serde_json::Value = serde_json::from_slice(binary).unwrap();
                    {
                        let mut mutex = arc_cloned.lock().unwrap();
                        match (*mutex).get_mut(json["code"].as_str().unwrap()) {
                            Some(a) => *a = json["trade_price"].as_f64().unwrap(),
                            None => {
                                (*mutex).insert(String::from(json["code"].as_str().unwrap()), json["trade_price"].as_f64().unwrap());
                            }
                        }
                        println!("{}", json["code"].as_str().unwrap());
                    }
                }
            }
        });
    }
}