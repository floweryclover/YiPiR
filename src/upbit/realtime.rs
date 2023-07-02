use websocket::{ClientBuilder, OwnedMessage, Message};
use crate::upbit::{UPBitError, UPBitSocket};

impl UPBitSocket {
    pub async fn start_realtime_data(&mut self) {
        if !self.realtime_running {
            let uuid = uuid::Uuid::new_v4().to_string();
            let json = serde_json::json!([
            {
                "ticket" : uuid,
            },
            {
                "type": "ticker",
                "codes": serde_json::Value::Array(self.get_all_available_tickers().await.unwrap().iter().map(|v| serde_json::json!(v)).collect()),
                "isOnlyRealtime": "true",
            },
            {
                "format": "DEFAULT"
            },
        ]);
            self.realtime_client.send_message(&Message::text(json.to_string())).unwrap();
        }
        self.realtime_running = true;
    }
    pub fn get_realtime_data(&mut self) -> Result<(), UPBitError>{
        if !self.realtime_running {
            return Err(UPBitError::OtherError(String::from("UPBitSocket.start_realtime_data() 를 먼저 호출하세요.")))
        }
        return match self.realtime_client.recv_message().unwrap() {
            //{"type":"ticker","code":"KRW-ETH","opening_price":2563000.0000,"high_price":2563000.0000,"low_price":2535000.0000,"trade_price":2544000.0000,"prev_closing_price":2563000.00000000,"acc_trade_price":10711749229.465390000000,"change":"FALL","change_price":19000.00000000,"signed_change_price":-19000.00000000,"change_rate":0.0074131877,"signed_change_rate":-0.0074131877,"ask_bid":"BID","trade_volume":0.48140949,"acc_trade_volume":4206.90085201,"trade_date":"20230702","trade_time":"094628","trade_timestamp":1688291188306,"acc_ask_volume":2500.15229731,"acc_bid_volume":1706.74855470,"highest_52_week_price":2795000.00000000,"highest_52_week_date":"2023-04-16","lowest_52_week_price":1345500.00000000,"lowest_52_week_date":"2022-07-13","market_state":"ACTIVE","is_trading_suspended":false,"delisting_date":null,"market_warning":"NONE","timestamp":1688291188337,"acc_trade_price_24h":21149832920.57972000,"acc_trade_volume_24h":8278.60582208,"stream_type":"REALTIME"}
            OwnedMessage::Binary(binary) => {
                let json = serde_json::from_slice::<serde_json::Value>(&binary).unwrap();
                Ok(())
            },
            _ => {
                Ok(())
            }
        }
    }
}