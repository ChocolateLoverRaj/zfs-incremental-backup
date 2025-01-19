# Written by ChatGPT
#!/bin/bash

# Step 1: Get the list of S3 buckets and SQS queues
buckets=$(aws s3 ls | cut -d" " -f 3)
queues=$(aws sqs list-queues --query "QueueUrls[]" --output text)

# Step 2: Display the resources to be deleted
echo "The following resources will be deleted:"
echo
echo "## S3 Buckets"
for bucket in $buckets; do
    echo "- $bucket"
done
echo
echo "## SQS Queues"
for queue in $queues; do
    echo "- $queue"
done
echo

# Step 3: Ask for confirmation
read -p "Are you sure you want to delete these resources? (y/n): " confirm

if [[ "$confirm" =~ ^[Yy]$ ]]; then
    # Step 4: Output Markdown for deleted resources
    echo "### Deleting Resources..."
    echo
    echo "# Deleted Resources"
    echo
    echo "## S3 Buckets"
    for bucket in $buckets; do
        echo "Deleting bucket: $bucket"
        aws s3 rb s3://"$bucket" --force
        echo "- $bucket"
    done
    echo
    echo "## SQS Queues"
    for queue in $queues; do
        echo "Deleting queue: $queue"
        aws sqs delete-queue --queue-url "$queue"
        echo "- $queue"
    done
    echo
    echo "All specified resources have been deleted."
else
    echo "Operation canceled. No resources were deleted."
fi
