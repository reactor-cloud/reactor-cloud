use crate::error::{EvalError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, info, warn};

/// Replay mode for test runs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Run against real provider
    Live,
    /// Record requests/responses to cassette
    Record,
    /// Replay from cassette
    Play,
}

/// A single request/response pair in a cassette
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CassetteEntry {
    pub request_hash: String,
    pub request_summary: String,
    pub response: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A recorded cassette of LLM interactions
#[derive(Debug, Clone, Default)]
pub struct Cassette {
    pub test_id: String,
    pub seed: String,
    pub model_pin: Option<String>,
    pub harness_revision: Option<String>,
    pub entries: HashMap<String, CassetteEntry>,
}

impl Cassette {
    pub fn new(test_id: impl Into<String>, seed: impl Into<String>) -> Self {
        Self {
            test_id: test_id.into(),
            seed: seed.into(),
            model_pin: None,
            harness_revision: None,
            entries: HashMap::new(),
        }
    }

    pub fn with_model_pin(mut self, pin: impl Into<String>) -> Self {
        self.model_pin = Some(pin.into());
        self
    }

    pub fn with_harness_revision(mut self, rev: impl Into<String>) -> Self {
        self.harness_revision = Some(rev.into());
        self
    }

    /// Add a request/response pair
    pub fn record(&mut self, request: &str, response: &str) {
        let hash = Self::hash_request(request);
        let entry = CassetteEntry {
            request_hash: hash.clone(),
            request_summary: Self::summarize_request(request),
            response: response.to_string(),
            timestamp: chrono::Utc::now(),
        };
        self.entries.insert(hash, entry);
    }

    /// Look up a response by request
    pub fn replay(&self, request: &str) -> Option<&str> {
        let hash = Self::hash_request(request);
        self.entries.get(&hash).map(|e| e.response.as_str())
    }

    /// Hash a request for lookup
    pub fn hash_request(request: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(request.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Create a short summary of a request for debugging
    fn summarize_request(request: &str) -> String {
        let truncated: String = request.chars().take(100).collect();
        if request.len() > 100 {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }
}

/// Manages cassette files on disk
pub struct CassetteManager {
    base_path: PathBuf,
}

impl CassetteManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    fn cassette_path(&self, test_id: &str, seed: &str) -> PathBuf {
        self.base_path.join(test_id).join(format!("{}.jsonl", seed))
    }

    /// Load a cassette from disk
    pub async fn load(&self, test_id: &str, seed: &str) -> Result<Cassette> {
        let path = self.cassette_path(test_id, seed);
        if !path.exists() {
            return Err(EvalError::CassetteNotFound(path.display().to_string()));
        }

        let file = tokio::fs::File::open(&path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let mut cassette = Cassette::new(test_id, seed);

        // First line is metadata
        if let Some(line) = lines.next_line().await? {
            if let Ok(meta) = serde_json::from_str::<CassetteMeta>(&line) {
                cassette.model_pin = meta.model_pin;
                cassette.harness_revision = meta.harness_revision;
            }
        }

        // Rest are entries
        while let Some(line) = lines.next_line().await? {
            if let Ok(entry) = serde_json::from_str::<CassetteEntry>(&line) {
                cassette.entries.insert(entry.request_hash.clone(), entry);
            }
        }

        info!("Loaded cassette with {} entries from {:?}", cassette.entries.len(), path);
        Ok(cassette)
    }

    /// Save a cassette to disk
    pub async fn save(&self, cassette: &Cassette) -> Result<()> {
        let path = self.cassette_path(&cassette.test_id, &cassette.seed);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(&path).await?;

        // Write metadata first
        let meta = CassetteMeta {
            test_id: cassette.test_id.clone(),
            seed: cassette.seed.clone(),
            model_pin: cassette.model_pin.clone(),
            harness_revision: cassette.harness_revision.clone(),
            entry_count: cassette.entries.len(),
        };
        let meta_line = serde_json::to_string(&meta)?;
        file.write_all(meta_line.as_bytes()).await?;
        file.write_all(b"\n").await?;

        // Write entries
        for entry in cassette.entries.values() {
            let line = serde_json::to_string(entry)?;
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        file.flush().await?;
        info!("Saved cassette with {} entries to {:?}", cassette.entries.len(), path);
        Ok(())
    }

    /// List all cassettes for a test
    pub async fn list_for_test(&self, test_id: &str) -> Result<Vec<String>> {
        let dir = self.base_path.join(test_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut seeds = Vec::new();
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    if let Some(s) = stem.to_str() {
                        seeds.push(s.to_string());
                    }
                }
            }
        }

        Ok(seeds)
    }

    /// Delete a cassette
    pub async fn delete(&self, test_id: &str, seed: &str) -> Result<()> {
        let path = self.cassette_path(test_id, seed);
        if path.exists() {
            fs::remove_file(&path).await?;
            debug!("Deleted cassette: {:?}", path);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CassetteMeta {
    test_id: String,
    seed: String,
    model_pin: Option<String>,
    harness_revision: Option<String>,
    entry_count: usize,
}
