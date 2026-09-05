use async_trait::async_trait;
use aws_sdk_s3::{Client, primitives::ByteStream};
use errors::infra::InfraError;
use serde::Deserialize;

#[async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put(&self, key: &str, content: Vec<u8>, content_type: &str) -> Result<(), InfraError>;
    async fn get(&self, key: &str) -> Result<Vec<u8>, InfraError>;
    async fn delete(&self, key: &str) -> Result<(), InfraError>;
}

#[derive(Deserialize)]
struct GarageConfig {
    endpoint: String,
    bucket: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
}

pub struct GarageObjectStorage {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl GarageObjectStorage {
    pub fn from_environment() -> anyhow::Result<Self> {
        let config: GarageConfig = envy::prefixed("S3_").from_env()?;
        let credentials = aws_sdk_s3::config::Credentials::new(
            config.access_key_id,
            config.secret_access_key,
            None,
            None,
            "seichi-portal",
        );
        let sdk_config = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .endpoint_url(config.endpoint)
            .region(aws_sdk_s3::config::Region::new(config.region))
            .credentials_provider(credentials)
            .force_path_style(true)
            .build();
        Ok(Self {
            client: Client::from_conf(sdk_config),
            bucket: config.bucket,
        })
    }

    fn request_error(operation: &str, error: impl std::fmt::Display) -> InfraError {
        InfraError::Outgoing {
            cause: format!("Garage {operation} request failed: {error}"),
        }
    }
}

#[async_trait]
impl ObjectStorage for GarageObjectStorage {
    async fn put(&self, key: &str, content: Vec<u8>, content_type: &str) -> Result<(), InfraError> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(content))
            .send()
            .await
            .map_err(|error| Self::request_error("put", error))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, InfraError> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| Self::request_error("get", error))?;
        response
            .body
            .collect()
            .await
            .map(|bytes| bytes.into_bytes().to_vec())
            .map_err(|error| Self::request_error("read", error))
    }

    async fn delete(&self, key: &str) -> Result<(), InfraError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| Self::request_error("delete", error))?;
        Ok(())
    }
}
