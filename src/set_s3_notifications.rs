use aws_config::SdkConfig;
use aws_sdk_s3::types::{
    builders::{NotificationConfigurationBuilder, QueueConfigurationBuilder},
    Event,
};

use crate::create_sqs::SqsArn;

pub async fn set_s3_notifications(
    sdk_config: &SdkConfig,
    bucket: &str,
    sqs_arn: &SqsArn,
) -> anyhow::Result<()> {
    let s3_client = aws_sdk_s3::Client::new(sdk_config);
    s3_client
        .put_bucket_notification_configuration()
        .bucket(bucket)
        .notification_configuration(
            NotificationConfigurationBuilder::default()
                .queue_configurations(
                    QueueConfigurationBuilder::default()
                        .id("Restore Completed")
                        .events(Event::S3ObjectRestoreCompleted)
                        .queue_arn(sqs_arn.to_string())
                        .build()
                        .unwrap(),
                )
                .build(),
        )
        .send()
        .await
        .unwrap();
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use aws_config::BehaviorVersion;

    use crate::{
        create_sqs::SqsArn, get_account_id::get_account_id,
        set_s3_notifications::set_s3_notifications,
    };

    #[tokio::test]
    async fn test() {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let bucket = "zfs-backup-d55d390a-a0c1-46de-b3e9-dbcedf643fe7";
        let queue_arn = SqsArn {
            region: "us-west-2".into(),
            account_id: get_account_id(&sdk_config).await.unwrap(),
            queue_name: "zfs-backup-016e60fd-638f-48b4-9875-a3b9b24c3a49".into(),
        };
        set_s3_notifications(&sdk_config, bucket, &queue_arn)
            .await
            .unwrap();
    }
}
