use aws_sdk_s3::types::StorageClass;

pub fn parse_storage_class(storage_class: &str) -> Result<StorageClass, String> {
    StorageClass::try_parse(storage_class).map_err(|e| e.to_string())
}
