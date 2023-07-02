use websocket::{ClientBuilder, OwnedMessage, Message};
use crate::upbit::UPBitAccount;

pub struct UPBitWebSocket {
    snapshot_client: websocket::client::sync::Client<Box<dyn websocket::stream::sync::NetworkStream + Send>>,
    realtime_client: websocket::client::sync::Client<Box<dyn websocket::stream::sync::NetworkStream + Send>>,
}

impl UPBitWebSocket {
    pub async fn new(upbit: &UPBitAccount) -> Self {

        let mut snapshot_client = ClientBuilder::new("wss://api.upbit.com/websocket/v1")
            .unwrap()
            .add_protocol("ping")
            .connect(None)
            .unwrap();


        let mut realtime_client = ClientBuilder::new("wss://api.upbit.com/websocket/v1")
            .unwrap()
            .add_protocol("ping")
            .connect(None)
            .unwrap();
        let uuid = uuid::Uuid::new_v4().to_string();
        let mut json = serde_json::json!([
            {
                "ticket" : uuid,
            },
            {
                "type": "ticker",
                "codes": serde_json::Value::Array(upbit.get_all_available_tickers().await.unwrap().iter().map(|v| serde_json::json!(v)).collect()),
                "isOnlyRealtime": "true",
            },
            {
                "format": "DEFAULT"
            },
        ]);
        realtime_client.send_message(&Message::text(json.to_string())).unwrap();

        UPBitWebSocket {
            snapshot_client,
            realtime_client,
        }
    }

    pub fn get_realtime_data(&mut self) {
        match self.realtime_client.recv_message().unwrap() {
            //{"type":"ticker","code":"KRW-ETH","opening_price":2563000.0000,"high_price":2563000.0000,"low_price":2535000.0000,"trade_price":2544000.0000,"prev_closing_price":2563000.00000000,"acc_trade_price":10711749229.465390000000,"change":"FALL","change_price":19000.00000000,"signed_change_price":-19000.00000000,"change_rate":0.0074131877,"signed_change_rate":-0.0074131877,"ask_bid":"BID","trade_volume":0.48140949,"acc_trade_volume":4206.90085201,"trade_date":"20230702","trade_time":"094628","trade_timestamp":1688291188306,"acc_ask_volume":2500.15229731,"acc_bid_volume":1706.74855470,"highest_52_week_price":2795000.00000000,"highest_52_week_date":"2023-04-16","lowest_52_week_price":1345500.00000000,"lowest_52_week_date":"2022-07-13","market_state":"ACTIVE","is_trading_suspended":false,"delisting_date":null,"market_warning":"NONE","timestamp":1688291188337,"acc_trade_price_24h":21149832920.57972000,"acc_trade_volume_24h":8278.60582208,"stream_type":"REALTIME"}
            OwnedMessage::Binary(binary) => {
                let json = serde_json::from_slice::<serde_json::Value>(&binary).unwrap();
                println!("{}", json.to_string())
            },
            _ => println!("No Data Received")
        }
    }
}