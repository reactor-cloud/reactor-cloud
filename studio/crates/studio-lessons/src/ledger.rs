use crate::error::{LessonError, Result};
use crate::lesson::{LessonId, Tier};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

/// An entry in the lesson ledger tracking citations and outcomes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub ts: DateTime<Utc>,
    pub lesson: LessonId,
    pub task: String,
    pub phase: String,
    pub cited: bool,
    pub outcome: LedgerOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LedgerOutcome {
    Success,
    Failure,
    Partial,
}

/// Writes entries to the ledger file (append-only)
pub struct LedgerWriter {
    path: std::path::PathBuf,
}

impl LedgerWriter {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Append an entry to the ledger
    pub async fn append(&self, entry: &LedgerEntry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        let line = serde_json::to_string(entry)?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        Ok(())
    }

    /// Read all entries from the ledger
    pub fn read_all(&self) -> Result<Vec<LedgerEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str(&line) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    return Err(LessonError::LedgerCorrupted {
                        line: line_num + 1,
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(entries)
    }

    /// Get entries for a specific lesson
    pub fn entries_for_lesson(&self, lesson_id: &LessonId) -> Result<Vec<LedgerEntry>> {
        Ok(self
            .read_all()?
            .into_iter()
            .filter(|e| &e.lesson == lesson_id)
            .collect())
    }

    /// Calculate statistics for a lesson from ledger entries
    pub fn lesson_stats(&self, lesson_id: &LessonId) -> Result<LessonStats> {
        let entries = self.entries_for_lesson(lesson_id)?;
        let cited_entries: Vec<_> = entries.iter().filter(|e| e.cited).collect();

        let citations = cited_entries.len() as u64;
        let successes = cited_entries
            .iter()
            .filter(|e| e.outcome == LedgerOutcome::Success)
            .count() as u64;
        let failures = cited_entries
            .iter()
            .filter(|e| e.outcome == LedgerOutcome::Failure)
            .count() as u64;

        Ok(LessonStats {
            citations,
            successes,
            failures,
            last_cited: entries.iter().filter(|e| e.cited).map(|e| e.ts).max(),
            last_success: entries
                .iter()
                .filter(|e| e.cited && e.outcome == LedgerOutcome::Success)
                .map(|e| e.ts)
                .max(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct LessonStats {
    pub citations: u64,
    pub successes: u64,
    pub failures: u64,
    pub last_cited: Option<DateTime<Utc>>,
    pub last_success: Option<DateTime<Utc>>,
}

impl LessonStats {
    pub fn success_rate(&self) -> f64 {
        if self.citations == 0 {
            0.0
        } else {
            self.successes as f64 / self.citations as f64
        }
    }
}
