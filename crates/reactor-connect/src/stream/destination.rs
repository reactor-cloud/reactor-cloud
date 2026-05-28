//! Destination sink trait and implementations.

use crate::descriptor::StreamDescriptor;
use crate::error::ConnectError;
use crate::protocol::AirbyteRecordMessage;
use async_trait::async_trait;

/// Destination sink for stream records.
#[async_trait]
pub trait DestinationSink: Send + Sync {
    /// Prepare the destination (create tables, etc.).
    async fn prepare(&self, streams: &[StreamDescriptor]) -> Result<(), ConnectError>;

    /// Write a batch of records.
    async fn write_batch(&self, records: &[AirbyteRecordMessage]) -> Result<u64, ConnectError>;

    /// Commit the transaction.
    async fn commit(&self) -> Result<(), ConnectError>;
}

/// Sink that writes to reactor-data.
pub struct ReactorDataSink {
    client: reqwest::Client,
    base_url: String,
    auth_token: String,
}

impl ReactorDataSink {
    /// Create a new reactor-data sink.
    pub fn new(base_url: String, auth_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            auth_token,
        }
    }
}

#[async_trait]
impl DestinationSink for ReactorDataSink {
    async fn prepare(&self, _streams: &[StreamDescriptor]) -> Result<(), ConnectError> {
        // TODO: Call data admin endpoint to create tables
        Ok(())
    }

    async fn write_batch(&self, records: &[AirbyteRecordMessage]) -> Result<u64, ConnectError> {
        let mut written = 0u64;
        
        // Group records by stream
        let mut by_stream: std::collections::HashMap<&str, Vec<&AirbyteRecordMessage>> =
            std::collections::HashMap::new();
        for record in records {
            by_stream
                .entry(&record.stream)
                .or_default()
                .push(record);
        }

        // Write each stream batch
        for (stream, stream_records) in by_stream {
            let rows: Vec<_> = stream_records.iter().map(|r| &r.data).collect();
            
            let resp = self.client
                .post(format!("{}/data/v1/{}", self.base_url, stream))
                .bearer_auth(&self.auth_token)
                .json(&serde_json::json!({ "rows": rows }))
                .send()
                .await?;

            if !resp.status().is_success() {
                return Err(ConnectError::Internal(format!(
                    "Data write failed: {}",
                    resp.status()
                )));
            }

            written += stream_records.len() as u64;
        }

        Ok(written)
    }

    async fn commit(&self) -> Result<(), ConnectError> {
        // reactor-data auto-commits
        Ok(())
    }
}

/// Sink that writes to reactor-storage.
pub struct ReactorStorageSink {
    client: reqwest::Client,
    base_url: String,
    auth_token: String,
    bucket: String,
    prefix: String,
}

impl ReactorStorageSink {
    /// Create a new reactor-storage sink.
    pub fn new(base_url: String, auth_token: String, bucket: String, prefix: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            auth_token,
            bucket,
            prefix,
        }
    }
}

#[async_trait]
impl DestinationSink for ReactorStorageSink {
    async fn prepare(&self, _streams: &[StreamDescriptor]) -> Result<(), ConnectError> {
        // Storage doesn't need preparation
        Ok(())
    }

    async fn write_batch(&self, records: &[AirbyteRecordMessage]) -> Result<u64, ConnectError> {
        if records.is_empty() {
            return Ok(0);
        }

        // Write as JSONL file
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let key = format!("{}/{}.jsonl", self.prefix, timestamp);
        
        let mut content = String::new();
        for record in records {
            content.push_str(&serde_json::to_string(&record.data)?);
            content.push('\n');
        }

        let resp = self.client
            .put(format!(
                "{}/storage/v1/object/{}/{}",
                self.base_url, self.bucket, key
            ))
            .bearer_auth(&self.auth_token)
            .body(content)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ConnectError::Internal(format!(
                "Storage write failed: {}",
                resp.status()
            )));
        }

        Ok(records.len() as u64)
    }

    async fn commit(&self) -> Result<(), ConnectError> {
        Ok(())
    }
}

/// Ephemeral sink for sandbox runs.
pub struct EphemeralSink {
    schema_name: String,
    inner: ReactorDataSink,
}

impl EphemeralSink {
    /// Create a new ephemeral sink.
    pub fn new(schema_name: String, inner: ReactorDataSink) -> Self {
        Self { schema_name, inner }
    }

    /// Get the schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }
}

#[async_trait]
impl DestinationSink for EphemeralSink {
    async fn prepare(&self, streams: &[StreamDescriptor]) -> Result<(), ConnectError> {
        // TODO: Create ephemeral schema
        self.inner.prepare(streams).await
    }

    async fn write_batch(&self, records: &[AirbyteRecordMessage]) -> Result<u64, ConnectError> {
        self.inner.write_batch(records).await
    }

    async fn commit(&self) -> Result<(), ConnectError> {
        self.inner.commit().await
    }
}
