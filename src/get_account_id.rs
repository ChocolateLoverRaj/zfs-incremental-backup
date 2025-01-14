use anyhow::anyhow;
use aws_config::SdkConfig;

pub async fn get_account_id(sdk_config: &SdkConfig) -> anyhow::Result<String> {
    Ok({
        let sts_client = aws_sdk_sts::Client::new(sdk_config);
        sts_client
            .get_caller_identity()
            .send()
            .await?
            .account
            .ok_or(anyhow!("No account id"))?
    })
}

#[cfg(test)]
mod tests {
    use aws_config::BehaviorVersion;

    use crate::get_account_id::get_account_id;

    #[tokio::test]
    async fn test_get_account_id() {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let account_id = get_account_id(&sdk_config).await.unwrap();
        println!("Account ID: {:?}", account_id);
    }
}
