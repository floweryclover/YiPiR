use serde_json::Value;

pub trait UPBitResponse {
    fn build(json: &Vec<Value>) -> Self;
}