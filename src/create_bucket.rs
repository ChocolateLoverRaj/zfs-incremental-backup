use std::fmt::Display;

use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::create_bucket::CreateBucketError;
use aws_sdk_s3::types::{BucketLocationConstraint, CreateBucketConfiguration};
use uuid::Uuid;

pub async fn create_bucket(
    s3_client: &aws_sdk_s3::Client,
    bucket_prefix: &impl Display,
    location: &BucketLocationConstraint,
) -> Result<String, SdkError<CreateBucketError, HttpResponse>> {
    let result = loop {
        let bucket_name = {
            let uuid = Uuid::new_v4();
            format!("{}-{}", bucket_prefix, uuid)
        };
        let result = s3_client
            .create_bucket()
            .bucket(&bucket_name)
            .create_bucket_configuration(
                CreateBucketConfiguration::builder()
                    .location_constraint(location.clone())
                    .build(),
            )
            .send()
            .await;
        match result {
            Ok(_bucket) => {
                break Ok(bucket_name);
            }
            Err(error) => match &error {
                SdkError::ServiceError(service_error) => match service_error.err() {
                    CreateBucketError::BucketAlreadyExists(_)
                    | CreateBucketError::BucketAlreadyOwnedByYou(_) => {
                        // Generate a new uuid on the next loop iteration
                    }
                    _ => break Err(error),
                },
                _ => break Err(error),
            },
        }
    };
    result
}
