use std::time::SystemTime;

use anyhow::{anyhow, Context};
use aws_config::BehaviorVersion;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct Price {
    USD: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize)]
struct Z3FQZG73HYSPVABR_JRTCKXETXF_PGHJ3S3EYE {
    #[serde(rename = "pricePerUnit")]
    price_per_unit: Price,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct PriceDensions {
    #[serde(rename = "Z3FQZG73HYSPVABR.JRTCKXETXF.PGHJ3S3EYE")]
    Z3FQZG73HYSPVABR_JRTCKXETXF_PGHJ3S3EYE: Z3FQZG73HYSPVABR_JRTCKXETXF_PGHJ3S3EYE,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
struct Z3FQZG73HYSPVABR_JRTCKXETXF {
    #[serde(rename = "priceDimensions")]
    price_dimensions: PriceDensions,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct Z3FQZG73HYSPVABR {
    #[serde(rename = "Z3FQZG73HYSPVABR.JRTCKXETXF")]
    Z3FQZG73HYSPVABR_JRTCKXETXF: Z3FQZG73HYSPVABR_JRTCKXETXF,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct OnDemand {
    Z3FQZG73HYSPVABR: Z3FQZG73HYSPVABR,
}

#[derive(Debug, Serialize, Deserialize)]
struct Terms {
    #[serde(rename = "OnDemand")]
    on_demand: OnDemand,
}

#[derive(Debug, Serialize, Deserialize)]
struct Prices {
    terms: Terms,
}

impl Prices {
    #[allow(unused)]
    pub async fn get(region: impl Into<String>) -> anyhow::Result<Self> {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            // The pricing api is only available in certain regions
            .region("us-east-1")
            .load()
            .await;
        let pricing_client = aws_sdk_pricing::Client::new(&sdk_config);

        let output = pricing_client
            .list_price_lists()
            .service_code("AmazonS3")
            .currency_code("USD")
            .effective_date(SystemTime::now().into())
            .region_code(region)
            .send()
            .await
            .context("Failed to get AWS pricing")?;
        let arn = output
            .price_lists()
            .first()
            .ok_or(anyhow!("No price list"))?
            .price_list_arn()
            .ok_or(anyhow!("No ARN"))?;
        let price_url = pricing_client
            .get_price_list_file_url()
            .price_list_arn(arn)
            .file_format("json")
            .send()
            .await?
            .url
            .ok_or(anyhow!("No S3 price URL"))?;
        println!("Got price URL: {:#?}", price_url);

        let prices = serde_json::from_str::<Value>(&reqwest::get(&price_url).await?.text().await?)?;
        println!("Prices: {:#?}", prices);
        let prices = serde_json::from_str::<Prices>(&reqwest::get(price_url).await?.text().await?)?;
        println!("Prices: {:#?}", prices);
        Ok(prices)
    }
}

// #[derive(Debug)]
// struct PricesRust {
//     standard_storage: Quantity<ISQ<uom::P2, Z0, Z0, Z0, Z0, Z0, Z0>, SI<f32>, f32>,
// }

// impl PricesRust {
//     pub fn test() {
//         let a = 1.0 / Information::new::<gigabyte>(1.0) / Time::new::<uom::si::time::second>(1.0);
//     }
// }
