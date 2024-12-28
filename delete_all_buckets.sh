# From https://github.com/aws/aws-cli/issues/2471#issuecomment-320444502
aws s3 ls | cut -d" " -f 3 | xargs -I{} aws s3 rb s3://{} --force
