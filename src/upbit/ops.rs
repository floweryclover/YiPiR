use std::ops::{BitAnd, BitOr};
use polars::series::ops::NullBehavior;
use polars::prelude::*;
use crate::upbit::{CandleData, CandleDataOperation};

pub fn get_rsi(candle_data: &[CandleData]) -> f64 {
    let rsi_series = get_rsi_series(candle_data);
    return match rsi_series.get(rsi_series.len()-1).unwrap() {
        AnyValue::Float64(f) => f,
        _ => panic!("형식이 잘못되었습니다.")
    };
}

pub fn get_rsi_series(candle_data: &[CandleData]) -> Series {
    candle_data
        .as_dataframe()
        .lazy()
        .select([
            col("price").diff(1, NullBehavior::Ignore).alias("diff"),
        ])
        .collect().unwrap()
        .lazy()
        .select([
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
        ])
        .collect().unwrap()
        .lazy()
        .select([
            (lit(100.0) - (lit(100.0) / (lit(1.0)+(col("au")/col("ad")) )))
                .alias("rsi")
        ]).collect().unwrap()
        .lazy().select([
        when(col("rsi").lt(lit(10)))
            .then(lit(-1.0))
            .otherwise(col("rsi"))
            .alias("rsi")
    ])
        .collect().unwrap()
        .column("rsi")
        .unwrap()
        .clone()
}

pub fn get_ewm_mean(candle_data: &[CandleData]) -> f64 {
    let ewm_mean_df = candle_data
        .as_dataframe()
        .lazy()
        .select([
            col("price")
                .ewm_mean(EWMOptions::default().and_com(13.0).and_min_periods(14))
                .alias("ewm_mean")
        ])
        .collect().unwrap();

    return match ewm_mean_df.get_columns()[0].get(ewm_mean_df.get_columns()[0].len()-1).unwrap() {
        AnyValue::Float64(f) => f,
        _ => panic!("형식이 잘못되었습니다.")
    };
}

pub fn get_std(candle_data: &[CandleData]) -> f64 {
    if let AnyValue::Float64(f) = candle_data
        .as_dataframe()
        .lazy()
        .select([
            col("price")
                .std(0)
                .alias("std")
        ])
        .collect().unwrap()
        .column("std").unwrap()
        .get(0).unwrap() {
        f
    } else {
        panic!("잘못된 형식입니다.")
    }
}

pub enum RsiDivergenceCheckMode {
    Peak,
    Minpoint,
}

/// # RSI 다이버전스 발생 확인
/// 주어진 Candle Data 슬라이스 내에서 RSI 다이버전스가 발생했으면 true를 반환합니다.
/// ### divergence_check_mode가 Peak인 경우
/// 가격이 상승세인 상황에서, bound를 넘는 RSI 고점 가장 최근 둘에 대해 RSI 고점은 하락, 가격 고점은 상승하는 경우 true를 반환합니다.
/// ### divergence_check_mode가 Minpoint인 경우
/// 가격이 하락세인 상황에서, bound 미만인 RSI 고점 가장 최근 둘에 대해 RSI 저점은 상승, 가격 저점은 하락하는 경우 true를 반환합니다.
pub fn check_rsi_divergence(candle_data: &[CandleData], divergence_check_mode: &RsiDivergenceCheckMode, rsi_bound: &f64, recent_data_bound: &usize) -> bool {
    let rsi_series = get_rsi_series(candle_data);

    match divergence_check_mode {
        RsiDivergenceCheckMode::Peak => {
            let mut peak_indexes = rsi_series
                .diff(1, NullBehavior::Ignore).unwrap()
                .gt(0.0).unwrap()
                .bitand(
                    rsi_series
                        .diff(-1, NullBehavior::Ignore).unwrap()
                        .gt(0.0).unwrap())
                .bitand(
                    rsi_series
                        .gt(*rsi_bound as f32).unwrap()
                )
                .into_series()
                .rechunk()
                .iter()
                .enumerate()
                .filter_map(
                    |(i, value)|
                        if let AnyValue::Boolean(is_minpoint) = value {
                            if is_minpoint {
                                Some(i)
                            } else {
                                None
                            }
                        } else {
                            None
                        })
                .collect::<Vec<usize>>();

            let right = peak_indexes.pop();
            let left = peak_indexes.pop();

            if let Some(right_index) = right {
                if let Some(left_index) = left {
                    // 가장 최근 두 저점에 대해 RSI와 가격을 가져옵니다.
                    let right_rsi = match rsi_series.get(right_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };
                    let left_rsi = match rsi_series.get(left_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };
                    let candle_df = candle_data.as_dataframe();
                    let right_price = match candle_df.column("price").unwrap().get(right_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };
                    let left_price = match candle_df.column("price").unwrap().get(left_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };

                    // 우측 저점이 최근 데이터 판정 기준보다 오래된 경우 false를 반환합니다.
                    if right_index < (rsi_series.len()-recent_data_bound) { return false; }
                    // 유효한 다이버전스 상황이 아니라면 false를 반환합니다.
                    if left_rsi < right_rsi || left_price > right_price { return false; }

                    // 다이버전스 상황 자체는 유효하지만, 추세의 반전을 의미하는지 확인합니다.
                    let past_df = candle_df.slice(0, left_index);
                    let past_price_delta_mean = match past_df.column("price").unwrap()
                        .diff(1, NullBehavior::Ignore).unwrap()
                        .mean() {
                        Some(f) => f,
                        None => return false
                    };

                    if past_price_delta_mean < 0.0 { return false; }

                    // 가격은 상승중이며 유효한 다이버전스가 발생하였습니다.
                    return true;
                }
            }
        }
        RsiDivergenceCheckMode::Minpoint => {
            let mut minpoint_indexes = rsi_series
                .diff(1, NullBehavior::Ignore).unwrap()
                .lt(0.0).unwrap()
                .bitand(
                    rsi_series
                        .diff(-1, NullBehavior::Ignore).unwrap()
                        .lt(0.0).unwrap())
                .bitand(
                    rsi_series
                        .lt(*rsi_bound as f32).unwrap()
                )
                .into_series()
                .rechunk()
                .iter()
                .enumerate()
                .filter_map(
                    |(i, value)|
                        if let AnyValue::Boolean(is_minpoint) = value {
                            if is_minpoint {
                                Some(i)
                            } else {
                                None
                            }
                        } else {
                            None
                        })
                .collect::<Vec<usize>>();

            let right = minpoint_indexes.pop();
            let left = minpoint_indexes.pop();

            if let Some(right_index) = right {
                if let Some(left_index) = left {
                    // 가장 최근 두 저점에 대해 RSI와 가격을 가져옵니다.
                    let right_rsi = match rsi_series.get(right_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };
                    let left_rsi = match rsi_series.get(left_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };
                    let candle_df = candle_data.as_dataframe();
                    let right_price = match candle_df.column("price").unwrap().get(right_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };
                    let left_price = match candle_df.column("price").unwrap().get(left_index).unwrap() {
                        AnyValue::Float64(f) => f,
                        _ => panic!("잘못된 형식입니다.")
                    };

                    // 우측 저점이 최근 데이터 판정 기준보다 오래된 경우 false를 반환합니다.
                    if right_index < (rsi_series.len()-recent_data_bound) { return false; }
                    // 유효한 다이버전스 상황이 아니라면 false를 반환합니다.
                    if left_rsi > right_rsi || left_price < right_price { return false; }

                    // 다이버전스 상황 자체는 유효하지만, 추세의 반전을 의미하는지 확인합니다.
                    let past_df = candle_df.slice(0, left_index);
                    let past_price_delta_mean = match past_df.column("price").unwrap()
                        .diff(1, NullBehavior::Ignore).unwrap()
                        .mean() {
                        Some(f) => f,
                        None => return false
                    };

                    if past_price_delta_mean > 0.0 { return false; }

                    // 가격은 하락중이며 유효한 다이버전스가 발생하였습니다.
                    return true;
                }
            }
        }
    }

    false
}
/// # RSI 꺾임 확인
/// 다음 값이 감소하며 rsi_bound를 넘는 RSI 지점을 검색합니다.
/// 즉 이전 RSI 수치가 rsi_bound보다 크고 다음 데이터 때 감소한 경우 true를 반환합니다.
/// 가장 최근 데이터는 확인하지 않고 직전 데이터까지만 판단합니다.
/// count 이내 데이터에 대해 확인하며, 3 이상 입력 데이터 크기 이하여야 합니다.
pub fn check_rsi_breaking_peak(candle_data: &[CandleData], count: &usize, rsi_bound: &f64) -> bool {
    if count > &candle_data.len() || count < &3 {
        panic!("bound값은 3 이상이며 입력 데이터의 크기까지만 허용됩니다.");
    }

    let full_rsi_series = get_rsi_series(candle_data);

    // .all()를 사용하기 위해 드모르간 법칙 이용
    // 조건에 맞는 값이 없는 경우 true true true true -> true -> NOT 연산으로 -> false
    // 존재하는 경우 true true false true -> false -> NOT 연산으로 -> true를 반환해야 합니다.
    !full_rsi_series
        .diff(-1, NullBehavior::Ignore).unwrap()
        .lt_eq(0.0).unwrap()
        .bitor(
            full_rsi_series
                .lt_eq(*rsi_bound as f32).unwrap()
        )
        .slice((candle_data.len()-count) as i64, count-1)
        .all()
}