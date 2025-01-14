#[cfg(test)]
pub mod tests {
    use aws_config::BehaviorVersion;
    use aws_sdk_s3::types::{
        builders::{GlacierJobParametersBuilder, RestoreRequestBuilder},
        Tier,
    };
    use aws_sdk_sqs::types::{builders::DeleteMessageBatchRequestEntryBuilder, QueueAttributeName};

    const QUEUE_URL: &str = "https://sqs.us-west-2.amazonaws.com/756723425372/test";

    #[tokio::test]
    async fn take_and_print_messages() {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let sqs_client = aws_sdk_sqs::Client::new(&sdk_config);
        let mut received_message_ids = Vec::default();
        loop {
            println!("Waiting to receive messages");
            let messages = sqs_client
                .receive_message()
                .queue_url(QUEUE_URL)
                .max_number_of_messages(10)
                .send()
                .await
                .unwrap()
                .messages
                .take()
                .unwrap_or_default();

            // For debugging purposes only
            let ignored_count = messages
                .iter()
                .filter(|message| match &message.message_id {
                    Some(id) => received_message_ids.contains(id),
                    None => false,
                })
                .count();

            // Ignore the already received messages. They should be deleted.
            let new_messages = messages
                .into_iter()
                .filter(|message| match &message.message_id {
                    Some(message_id) => !received_message_ids.contains(message_id),
                    None => false,
                })
                .collect::<Vec<_>>();

            // Add the new messages to the list of received messages
            received_message_ids.extend(
                new_messages
                    .iter()
                    .map(|message| message.message_id())
                    .filter_map(|message_id| message_id)
                    .map(|message_id| message_id.to_owned()),
            );

            // Delete these messages from the queue
            let messages_to_delete_from_queue = new_messages
                .iter()
                .map(|message| {
                    DeleteMessageBatchRequestEntryBuilder::default()
                        .id(message.message_id().unwrap())
                        .receipt_handle(message.receipt_handle().unwrap())
                        .build()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            if !messages_to_delete_from_queue.is_empty() {
                sqs_client
                    .delete_message_batch()
                    .queue_url(QUEUE_URL)
                    .set_entries(Some(messages_to_delete_from_queue))
                    .send()
                    .await
                    .unwrap();
            }

            // Get the message bodies for debugging purposes
            let message_bodies = new_messages
                .into_iter()
                .map(|message| message.body)
                .filter_map(|message| message)
                .collect::<Vec<_>>();
            println!(
                "new messages: {:?} (ignored {} messages that were already received)",
                message_bodies, ignored_count
            );
        }
    }

    #[tokio::test]
    async fn get_queue_attributes() {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let sqs_client = aws_sdk_sqs::Client::new(&sdk_config);
        let attributes = sqs_client
            .get_queue_attributes()
            .attribute_names(QueueAttributeName::VisibilityTimeout)
            .queue_url(QUEUE_URL)
            .send()
            .await
            .unwrap();
        println!("{:#?}", attributes);
    }

    #[tokio::test]
    async fn restore_to_new_object() {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let s3_client = aws_sdk_s3::Client::new(&sdk_config);
        let bucket = "zfs-backup-36486ac0-7b1a-4068-b429-06a61fef623d";
        let output = s3_client
            .restore_object()
            .bucket(bucket)
            .key("ChocolateLoverRaj.jpeg")
            .restore_request(
                RestoreRequestBuilder::default()
                    .glacier_job_parameters(
                        GlacierJobParametersBuilder::default()
                            .tier(Tier::Bulk)
                            .build()
                            .unwrap(),
                    )
                    .days(1)
                    // .output_location(
                    //     OutputLocationBuilder::default()
                    //         .s3(S3LocationBuilder::default()
                    //             .bucket_name(bucket)
                    //             .prefix("restored-")
                    //             .storage_class(StorageClass::Standard)
                    //             .build()
                    //             .unwrap())
                    //         .build(),
                    // )
                    .build(),
            )
            .send()
            .await
            .unwrap();
        println!("{:#?}", output);
    }

    #[tokio::test]
    async fn copy_object() {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let s3_client = aws_sdk_s3::Client::new(&sdk_config);
        let bucket = "zfs-backup-36486ac0-7b1a-4068-b429-06a61fef623d";
        let output = s3_client
            .copy_object()
            .bucket(bucket)
            .copy_source(format!(
                "{}/PXL_20241215_190954765_exported_1776_1734289812865~2.jpg",
                bucket
            ))
            .key("copied.jpg")
            .send()
            .await
            .unwrap();
        println!("{:#?}", output);
    }
}
