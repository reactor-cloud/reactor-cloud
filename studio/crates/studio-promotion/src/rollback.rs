use crate::error::{PromotionError, Result};
use studio_lessons::{Lesson, LessonId, LessonStore, Tier};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{info, warn};

/// Record of a rollback event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub iteration: u32,
    pub lessons_rolled_back: Vec<LessonId>,
    pub pass_rate_before: f64,
    pub pass_rate_after: f64,
    pub timestamp: DateTime<Utc>,
}

/// The rollback gate prevents lessons from persisting if they don't improve pass rates
pub struct RollbackGate {
    store: LessonStore,
    staged_this_iteration: HashSet<LessonId>,
}

impl RollbackGate {
    pub fn new(store: LessonStore) -> Self {
        Self {
            store,
            staged_this_iteration: HashSet::new(),
        }
    }

    /// Mark a lesson as staged in the current iteration
    pub fn mark_staged(&mut self, lesson_id: LessonId) {
        self.staged_this_iteration.insert(lesson_id);
    }

    /// Clear the staged set (call at the start of each iteration)
    pub fn clear_staged(&mut self) {
        self.staged_this_iteration.clear();
    }

    /// Get the lessons staged this iteration
    pub fn staged_lessons(&self) -> &HashSet<LessonId> {
        &self.staged_this_iteration
    }

    /// Check if rollback is needed and perform it if so
    ///
    /// Returns a RollbackRecord if rollback was performed, None otherwise
    pub async fn check(
        &mut self,
        iteration: u32,
        pass_rate_before: f64,
        pass_rate_after: f64,
    ) -> Result<Option<RollbackRecord>> {
        // If pass rate improved or stayed the same with no lessons, no rollback needed
        if pass_rate_after > pass_rate_before || self.staged_this_iteration.is_empty() {
            info!(
                "Iteration {}: Pass rate {:.1}% -> {:.1}%, no rollback needed",
                iteration,
                pass_rate_before * 100.0,
                pass_rate_after * 100.0
            );
            return Ok(None);
        }

        // Pass rate didn't improve - roll back lessons staged this iteration
        warn!(
            "Iteration {}: Pass rate did not improve ({:.1}% -> {:.1}%), rolling back {} lessons",
            iteration,
            pass_rate_before * 100.0,
            pass_rate_after * 100.0,
            self.staged_this_iteration.len()
        );

        let lessons_to_rollback: Vec<_> = self.staged_this_iteration.iter().cloned().collect();

        for lesson_id in &lessons_to_rollback {
            match self.store.load(lesson_id).await {
                Ok(lesson) => {
                    // Delete the lesson (or could move back to T0)
                    if let Err(e) = self.store.delete(&lesson).await {
                        warn!("Failed to rollback lesson {}: {}", lesson_id, e);
                    } else {
                        info!("Rolled back lesson {}", lesson_id);
                    }
                }
                Err(e) => {
                    warn!("Failed to load lesson {} for rollback: {}", lesson_id, e);
                }
            }
        }

        self.staged_this_iteration.clear();

        Ok(Some(RollbackRecord {
            iteration,
            lessons_rolled_back: lessons_to_rollback,
            pass_rate_before,
            pass_rate_after,
            timestamp: Utc::now(),
        }))
    }

    /// Force rollback of specific lessons
    pub async fn rollback_lessons(&self, lesson_ids: &[LessonId]) -> Result<Vec<LessonId>> {
        let mut rolled_back = Vec::new();

        for lesson_id in lesson_ids {
            match self.store.load(lesson_id).await {
                Ok(lesson) => {
                    if let Err(e) = self.store.delete(&lesson).await {
                        warn!("Failed to rollback lesson {}: {}", lesson_id, e);
                    } else {
                        info!("Rolled back lesson {}", lesson_id);
                        rolled_back.push(lesson_id.clone());
                    }
                }
                Err(e) => {
                    warn!("Failed to load lesson {} for rollback: {}", lesson_id, e);
                }
            }
        }

        Ok(rolled_back)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_rollback_on_regression() {
        let temp_dir = TempDir::new().unwrap();
        let store = LessonStore::new(temp_dir.path().join("lessons"));
        store.init().await.unwrap();

        let mut gate = RollbackGate::new(store);

        // Simulate staging a lesson
        let lesson_id = LessonId::from_string("test-lesson");
        gate.mark_staged(lesson_id.clone());

        // Check with regression
        let result = gate.check(1, 0.8, 0.7).await.unwrap();

        assert!(result.is_some());
        let record = result.unwrap();
        assert_eq!(record.iteration, 1);
        assert!(record.lessons_rolled_back.contains(&lesson_id));
    }

    #[tokio::test]
    async fn test_no_rollback_on_improvement() {
        let temp_dir = TempDir::new().unwrap();
        let store = LessonStore::new(temp_dir.path().join("lessons"));
        store.init().await.unwrap();

        let mut gate = RollbackGate::new(store);

        // Simulate staging a lesson
        gate.mark_staged(LessonId::from_string("test-lesson"));

        // Check with improvement
        let result = gate.check(1, 0.7, 0.8).await.unwrap();

        assert!(result.is_none());
    }
}
