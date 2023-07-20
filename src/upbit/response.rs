use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;
use polars::prelude::*;
use crate::upbit::ops::*;

#[derive(Deserialize, Debug)]
pub struct Balance {
    pub currency: String,
    #[serde(deserialize_with = "f64_from_str")]
    pub balance: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub locked: f64,
    #[serde(deserialize_with = "f64_from_str")]
    pub avg_buy_price: f64,
    pub avg_buy_price_modified: bool,
    pub unit_currency: String,
}

#[derive(Deserialize, Debug)]
pub struct CandleData {
    pub market: String,
    pub candle_date_time_utc: String,
    pub candle_date_time_kst: String,
    pub opening_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub trade_price: f64,
    pub timestamp: i64,
    pub candle_acc_trade_price: f64,
    pub candle_acc_trade_volume: f64,
    pub unit: i32,
}

pub trait CandleDataOperation {
    fn as_dataframe(&self) -> DataFrame;
    fn get_rsi(&self) -> f64;
    fn get_ewm_mean(&self) -> f64;
    fn get_std(&self) -> f64;
    fn get_last_price(&self) -> f64;
    fn check_rsi_divergence(&self, divergence_check_mode: &RsiDivergenceCheckMode, rsi_bound: &f64, recent_data_bound: &usize) -> bool;
}

impl CandleDataOperation for Vec<CandleData> {
    fn as_dataframe(&self) -> DataFrame {
        self.as_slice().as_dataframe()
    }

    fn get_rsi(&self) -> f64 {
        self.as_slice().get_rsi()
    }

    fn get_ewm_mean(&self) -> f64 {
        self.as_slice().get_ewm_mean()
    }

    fn get_std(&self) -> f64 { self.as_slice().get_std() }

    fn get_last_price(&self) -> f64 {
        self.as_slice().get_last_price()
    }

    fn check_rsi_divergence(&self, divergence_check_mode: &RsiDivergenceCheckMode, rsi_bound: &f64, recent_data_bound: &usize) -> bool { self.as_slice().check_rsi_divergence(divergence_check_mode, rsi_bound, recent_data_bound) }
}

impl CandleDataOperation for &[CandleData] {
    fn as_dataframe(&self) -> DataFrame {
        let mut timestamps = Vec::new();
        let mut prices = Vec::new();
        let mut volumes = Vec::new();

        self.iter()
            .rev()
            .for_each(|data| {
                timestamps.push(data.timestamp);
                prices.push(data.trade_price);
                volumes.push(data.candle_acc_trade_volume);
            });

        let timestamp_series = Series::from_vec("timestamp", timestamps);
        let price_series = Series::from_vec("price", prices);
        let volume_series = Series::from_vec("volume", volumes);

        DataFrame::new(vec![timestamp_series, price_series, volume_series]).unwrap()
    }

    fn get_rsi(&self) -> f64 {
        get_rsi(self)
    }

    fn get_ewm_mean(&self) -> f64 {
        get_ewm_mean(self)
    }

    fn get_std(&self) -> f64 { get_std(self) }

    fn get_last_price(&self) -> f64 {
        self
            .last().unwrap()
            .trade_price
    }

    fn check_rsi_divergence(&self, divergence_check_mode: &RsiDivergenceCheckMode, rsi_bound: &f64, recent_data_bound: &usize) -> bool {
        check_rsi_divergence(self, divergence_check_mode, rsi_bound, recent_data_bound)
    }
}

#[derive(Deserialize, Debug)]
pub struct Ticker {
    pub market: String,
    pub korean_name: String,
    pub english_name: String,
}

fn f64_from_str<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where D: Deserializer<'de> {
    let s = String::deserialize(deserializer).unwrap();
    f64::from_str(&s).map_err(de::Error::custom)
}