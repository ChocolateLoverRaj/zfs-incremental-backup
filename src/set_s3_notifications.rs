use anyhow::{anyhow, Context};
use aws_config::SdkConfig;
use aws_sdk_s3::types::{
    builders::{NotificationConfigurationBuilder, QueueConfigurationBuilder},
    Event,
};
use serde::{Deserialize, Serialize};

use crate::create_sqs::SqsArn;

/// Sets the S3 notification and waits for the test event
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
    let sqs_client = aws_sdk_sqs::Client::new(&sdk_config);
    let message = loop {
        if let Some(message) = sqs_client
            .receive_message()
            .queue_url(sqs_arn.get_url())
            .max_number_of_messages(1)
            .send()
            .await?
            .messages
            .take()
            .unwrap_or_default()
            .into_iter()
            .next()
        {
            break message;
        }
    };
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct SqsS3TestMessage {
        service: String,
        event: String,
        time: String,
        bucket: String,
        request_id: String,
        host_id: String,
    }
    let message_body = serde_json::from_str::<SqsS3TestMessage>(&message.body.unwrap_or_default())
        .context("Invalid test SQS message")?;
    if message_body.event != "s3:TestEvent" {
        Err(anyhow!("Unexpected message event"))?;
    }
    if message_body.bucket != bucket {
        Err(anyhow!("Bucket does not match"))?;
    }
    sqs_client
        .delete_message()
        .queue_url(sqs_arn.get_url())
        .receipt_handle(
            message
                .receipt_handle
                .ok_or(anyhow!("No message receipt handle"))?,
        )
        .send()
        .await?;
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
