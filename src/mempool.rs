use reqwest::Client;
use serde_json::Value;

pub async fn get_fastest_fee() -> Result<u8, reqwest::Error> {
    const API_MEMPOOL: &'static str = "https://mempool.space/api/v1/fees/recommended";

    let res: Value = Client::new().get(API_MEMPOOL).send().await?.json().await?;

    Ok(res["fastestFee"].as_u64().unwrap() as u8)
}
