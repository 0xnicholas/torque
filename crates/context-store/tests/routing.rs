use context_store::{route_storage, StorageType};

#[test]
fn test_route_storage_small_json_redis() {
    let data = vec![0u8; 100];
    assert_eq!(
        route_storage(data.len(), "application/json"),
        StorageType::Redis
    );
}

#[test]
fn test_route_storage_small_non_json_s3() {
    let data = vec![0u8; 100];
    assert_eq!(route_storage(data.len(), "text/plain"), StorageType::S3);
}

#[test]
fn test_route_storage_medium_json_s3() {
    let data = vec![0u8; 300 * 1024];
    assert_eq!(
        route_storage(data.len(), "application/json"),
        StorageType::S3
    );
}

#[test]
fn test_route_storage_large_s3() {
    let data = vec![0u8; 15 * 1024 * 1024];
    assert_eq!(
        route_storage(data.len(), "application/json"),
        StorageType::S3
    );
}

#[test]
fn test_route_storage_at_threshold() {
    let data = vec![0u8; 256 * 1024];
    assert_eq!(
        route_storage(data.len(), "application/json"),
        StorageType::S3
    );
}

#[test]
fn test_route_storage_just_under_threshold() {
    let data = vec![0u8; 255 * 1024];
    assert_eq!(
        route_storage(data.len(), "application/json"),
        StorageType::Redis
    );
}
