use std::fmt::Display;

use aws_config::SdkConfig;
use aws_sdk_s3::types::BucketLocationConstraint;
use aws_sdk_sqs::error::SdkError;
use aws_sdk_sqs::operation::create_queue::CreateQueueError;
use aws_sdk_sqs::types::QueueAttributeName;
use serde_json::json;
use uuid::Uuid;

use crate::get_account_id::get_account_id;

pub struct SqsArn {
    pub region: String,
    pub account_id: String,
    pub queue_name: String,
}

impl Display for SqsArn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "arn:aws:sqs:{}:{}:{}",
            self.region, self.account_id, self.queue_name
        )
    }
}

pub async fn create_sqs(
    sdk_config: &SdkConfig,
    queue_prefix: &impl Display,
    s3_bucket: &str,
    region: &BucketLocationConstraint,
) -> anyhow::Result<SqsArn> {
    Ok({
        let sqs_client = aws_sdk_sqs::Client::new(sdk_config);
        let account_id = get_account_id(&sdk_config).await?;
        loop {
            let queue_name = {
                let uuid = Uuid::new_v4();
                format!("{}-{}", queue_prefix, uuid)
            };
            let sqs_arn = SqsArn {
                account_id: account_id.clone(),
                region: region.to_string(),
                queue_name: queue_name.clone(),
            };
            let result = sqs_client
                .create_queue()
                .queue_name(&queue_name)
                .attributes(QueueAttributeName::VisibilityTimeout, 30.to_string())
                .attributes(
                    QueueAttributeName::MessageRetentionPeriod,
                    1_209_600.to_string(),
                )
                .attributes(QueueAttributeName::DelaySeconds, 0.to_string())
                .attributes(QueueAttributeName::MaximumMessageSize, 256_000.to_string())
                .attributes(
                    QueueAttributeName::ReceiveMessageWaitTimeSeconds,
                    20.to_string(),
                )
                .attributes(QueueAttributeName::Policy, {
                    json!({
                        "Version": "2012-10-17",
                        "Statement": [
                            {
                                "Effect": "Allow",
                                "Principal": {
                                    "Service": "s3.amazonaws.com"
                                },
                                "Action": "SQS:SendMessage",
                                "Resource": sqs_arn.to_string(),
                                "Condition": {
                                    "ArnLike": {
                                        "aws:SourceArn": format!("arn:aws:s3:*:*:{}", s3_bucket)
                                    }
                                }
                            }
                        ]
                    })
                    .to_string()
                })
                .send()
                .await;
            match result {
                Ok(_bucket) => {
                    break Ok(sqs_arn);
                }
                Err(error) => match &error {
                    SdkError::ServiceError(service_error) => match service_error.err() {
                        CreateQueueError::QueueNameExists(_) => {
                            // Generate a new uuid on the next loop iteration
                        }
                        _ => break Err(error),
                    },
                    _ => break Err(error),
                },
            }
        }?
    })
}
