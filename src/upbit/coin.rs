use polars::prelude::*;
use crate::upbit::{UPBitError, UPBitSocket};

pub struct Coin {
    ticker: String,
    dataframe: DataFrame,
}

pub enum CoinError {
    WrongDataFrameError(String),
    DataFrameNotInitializedError,
}

impl std::fmt::Debug for CoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CoinError::WrongDataFrameError(detail) => write!(f, "잘못된 DataFrame이 입력되었습니다: {}", detail),
            CoinError::DataFrameNotInitializedError => write!(f, "Coin 객체의 DataFrame을 설정하지 않은 상태로 작업을 시도했습니다."),
        }
    }
}

impl std::fmt::Display for CoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CoinError::WrongDataFrameError(detail) => write!(f, "잘못된 DataFrame이 입력되었습니다: {}", detail),
            CoinError::DataFrameNotInitializedError => write!(f, "Coin 객체의 DataFrame을 설정하지 않은 상태로 작업을 시도했습니다."),
        }
    }
}

impl UPBitSocket {
    // 5분 이상 텀 두고 반복 호출 권장
    pub async fn refresh_recommended_coins(&mut self) -> Result<(f64, f64, f64), UPBitError> {
        // 이전에 저장된 BEI Delta값이 0.0이면 미설정되었음을 의미하므로 이전값을 넣는다
        let (prev_delta, prev_bersi) = if (self.previous_bei_delta).abs() < 0.0001 {
            match self.btc_eth_indicator(1).await {
                Ok((_, b, c)) => (b, c),
                Err(e) => return Err(e),
            }
        } else {
            (self.previous_bei_delta, self.previous_bersi)
        };

        let (bei, bei_delta, bersi) = match self.btc_eth_indicator(0).await {
            Ok((a, b, c)) => (a, b, c),
            Err(e) => return Err(e),
        };
        self.previous_bei_delta = bei_delta;
        // BEI 하락장이면 구매를 하지 않는 선택 내림
        if bei_delta < 0.0 {
            if !self.recommended_coins.is_empty() {
                self.recommended_coins.clear();
            }

        } else {
            // 상승장으로의 전환일 때 선택하는 것이 현명할 듯
            if prev_delta < 0.0 && bersi < 45.0 {
                let heavy_tickers = match self.get_tickers_sortby_volume().await {
                    Ok(vec) => vec,
                    Err(e) => return Err(e),
                };

                let mut tickers_map = std::collections::HashMap::new();
                // 현재 저점인 종목만 추리기
                let mut count = 1;
                for ticker in heavy_tickers {
                    if count > 20 {
                        break;
                    }
                   let df = match self.get_recent_market_data(ticker.as_str(), 200).await {
                       Ok(df) => df,
                       Err(_) => panic!("refresh_recommended_coins()에서 Too Many Request Error 이외의 오류가 발생했습니다.")
                   };
                    // 개별적으로 따지는게 나을듯
                    // let df_minus_std = df.clone().lazy().select([
                    //     ((col("trade_price")-col("trade_price").std(0))/(col("trade_price").ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14)))).alias("delta")
                    // ]).collect().unwrap();
                    //
                    // let last = match df_minus_std.get_columns()[0].get(df_minus_std.get_columns().len()-1).unwrap() {
                    //     AnyValue::Float64(f) => f,
                    //     _ => 1.0,
                    // };
                    //
                    // if last > 0.0 {
                    //     println!("{} 는 상승장입니다({})", ticker, last);
                    //     continue;
                    // }
                    println!("{}", ticker);
                    let mut coin = Coin::new(ticker.as_str());
                    coin.init_data(&df);
                    tickers_map.insert(ticker, coin);
                    count += 1;
                }

                self.recommended_coins.clear();
                self.recommended_coins = tickers_map;
            } else {
                if !self.recommended_coins.is_empty() {
                    self.recommended_coins.clear();
                }
            }
        }
        println!("{:?}", self.recommended_coins.keys());
        Ok((bei, bei_delta, bersi))
    }
}

impl Coin {
    pub fn new(ticker: &str) -> Self {
        Coin {
            ticker: String::from(ticker),
            dataframe: DataFrame::empty(),
        }
    }

    pub fn init_data(&mut self, dataframe: &DataFrame) {
        self.dataframe = dataframe
            .clone()
            .lazy()
            .select([
                col("timestamp"),
                col("candle_date_time_kst"),
                col("high_price"),
                col("low_price"),
                col("opening_price"),
                col("trade_price"),
            ])
            .collect()
            .unwrap()
            .sort(["timestamp"], false)
            .unwrap();
    }

    pub fn update_data(&mut self, one_row: &DataFrame) -> Result<(), CoinError> {
        if one_row.get_columns()[0].len() > 1 {
            return Err(CoinError::WrongDataFrameError(String::from("update_data()에 입력된 one_row 매개변수의 행 크기가 1보다 큽니다.")))
        }
        self.dataframe = self.dataframe
            .shift(-1)
            .drop_nulls::<String>(None).unwrap();
        self.dataframe.extend(one_row).unwrap();

        Ok(())
    }

    pub fn get_dataframe(&self) -> &DataFrame {
        &self.dataframe
    }

    // previous 만큼 과거 데이터를 가져옴 (0은 현재)
    pub async fn get_rsi(&self, realtime_data_arc: &Arc<std::sync::Mutex<std::collections::HashMap<String, f64>>>, previous: u8) -> Result<f64, CoinError> {
        use polars::series::ops::NullBehavior;

        if self.dataframe.is_empty() {
            return Err(CoinError::DataFrameNotInitializedError);
        }


        let arc = realtime_data_arc.clone();
        let mut prices = self.dataframe.clone().lazy().select([col("trade_price")]).collect().unwrap();
        let realtime_price = {
            let mutex = arc.lock().unwrap();
            if (*mutex).contains_key(self.ticker.as_str()) {
                (*mutex).get(self.ticker.as_str()).unwrap().clone()
            } else {
                0.0
            }
        };
        let realtime_row: DataFrame =  df!("trade_price" => &[realtime_price]).unwrap();
        prices.extend(&realtime_row).unwrap();

        let au_ad = prices.lazy().select([
            col("trade_price").diff(1, NullBehavior::Ignore).alias("diff"),
        ])
            .collect().unwrap()
            .lazy().select([
            when(col("diff").lt(lit(0)))
                .then(lit(0))
                .otherwise(col("diff"))
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("au"),
            when(col("diff").gt(lit(0)))
                .then(lit(0))
                .otherwise(col("diff"))
                .abs()
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("ad"),
        ]).collect().unwrap();

        let rsi_df = au_ad.lazy().select([
            (lit(100.0) - (lit(100.0) / (lit(1.0)+(col("au")/col("ad")) )))
                .alias("rsi")
        ]).collect().unwrap()
            .lazy().select([
            when(col("rsi").lt(lit(10)))
                .then(lit(-1.0))
                .otherwise(col("rsi"))
                .alias("rsi")
        ]).collect().unwrap();

        let rsi = match rsi_df.get_columns()[0].get(rsi_df.get_columns()[0].len()-1-(previous as usize)).unwrap() {
            AnyValue::Float64(f) => f,
            _ => -1.0
        };
        return Ok(rsi);
    }

    // 구매 판단
    pub async fn buy_decision(&self, realtime_data_arc: &Arc<std::sync::Mutex<std::collections::HashMap<String, f64>>>) -> bool {
        let prev_rsi = self.get_rsi(realtime_data_arc, 1).await.unwrap();
        let current_rsi = self.get_rsi(realtime_data_arc, 0).await.unwrap();

        if prev_rsi < 0.0 || current_rsi < 0.0 {
            return false;
        }

        // if !self.realtime_price.contains_key(ticker.as_str()) {
        //     continue;
        // }
        // let realtime_price = self.realtime_price[ticker.as_str()];

        let df = self.get_dataframe();

        let values = df.clone().lazy().select([
            col("trade_price").alias("price"),
            col("trade_price").ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14)).alias("mean")
        ]).collect().unwrap();

        let current_price = match values.column("price").unwrap().get(values.get_columns()[0].len()-1).unwrap() {
            AnyValue::Float64(f) => f,
            _ => 0.0,
        };
        let mean = match values.column("mean").unwrap().get(values.get_columns()[0].len()-1).unwrap() {
            AnyValue::Float64(f) => f,
            _ => 0.0,
        };

        // - 0.5 % 부터 구매해보자
        let ratio = 100.0*(current_price-mean)/mean;
        // 준비된 지표들
        // 현재가
        // 평균(이동평균)
        // 현재 RSI
        // 직전 RSI

        // 사용 지표들
        // 현재가 - ratio 0.5%로 사용
        // 평균 = ratio 0.5%로 사용
        // 현재 RSI > 35 직전 RSI < 35 사용
        return ratio < -0.5 && current_rsi > 35.0 && prev_rsi < 35.0;
    }
}