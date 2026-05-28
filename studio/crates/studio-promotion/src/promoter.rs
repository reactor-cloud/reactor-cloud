use crate::error::{PromotionError, Result};
use studio_lessons::{LedgerEntry, LedgerOutcome, LedgerWriter, Lesson, LessonId, LessonStore, Tier};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// A tier transition event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierTransition {
    pub lesson_id: LessonId,
    pub from_tier: Tier,
    pub to_tier: Tier,
    pub reason: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Configuration for promotion rules
#[derive(Debug, Clone)]
pub struct PromotionConfig {
    /// Minimum citations for T1 -> T2 promotion
    pub min_citations_t1_t2: u64,
    /// Minimum success rate for T1 -> T2 promotion
    pub min_success_rate_t1_t2: f64,
    /// Success rate threshold for demotion
    pub demotion_success_rate_threshold: f64,
    /// Minimum citations before demotion can occur
    pub demotion_min_citations: u64,
    /// Days of inactivity before considering demotion
    pub demotion_inactive_days: i64,
}

impl Default for PromotionConfig {
    fn default() -> Self {
        Self {
            min_citations_t1_t2: 1,
            min_success_rate_t1_t2: 0.5,
            demotion_success_rate_threshold: 0.5,
            demotion_min_citations: 5,
            demotion_inactive_days: 30,
        }
    }
}

/// The promoter handles tier transitions based on ledger data
pub struct Promoter {
    store: LessonStore,
    ledger: LedgerWriter,
    config: PromotionConfig,
}

impl Promoter {
    pub fn new(store: LessonStore, ledger: LedgerWriter) -> Self {
        Self {
            store,
            ledger,
            config: PromotionConfig::default(),
        }
    }

    pub fn with_config(mut self, config: PromotionConfig) -> Self {
        self.config = config;
        self
    }

    /// Run a promotion tick - check all lessons and apply tier transitions
    pub async fn tick(&self) -> Result<Vec<TierTransition>> {
        info!("Running promotion tick");
        let mut transitions = Vec::new();

        // Load all lessons
        let lessons = self.store.list_all().await?;

        // Load ledger entries
        let entries = self.ledger.read_all()?;

        // Build stats per lesson
        let stats = self.build_lesson_stats(&entries);

        for mut lesson in lessons {
            // Check for promotion
            if let Some(transition) = self.check_promotion(&lesson, &stats).await? {
                transitions.push(transition);
            }

            // Check for demotion
            if let Some(transition) = self.check_demotion(&lesson, &stats).await? {
                transitions.push(transition);
            }
        }

        info!("Promotion tick complete: {} transitions", transitions.len());
        Ok(transitions)
    }

    fn build_lesson_stats(&self, entries: &[LedgerEntry]) -> HashMap<LessonId, LessonStats> {
        let mut stats: HashMap<LessonId, LessonStats> = HashMap::new();

        for entry in entries {
            let lesson_stats = stats.entry(entry.lesson.clone()).or_default();

            if entry.cited {
                lesson_stats.citations += 1;
                match entry.outcome {
                    LedgerOutcome::Success => lesson_stats.successes += 1,
                    LedgerOutcome::Failure => lesson_stats.failures += 1,
                    LedgerOutcome::Partial => {}
                }
                if lesson_stats.last_cited.map(|t| t < entry.ts).unwrap_or(true) {
                    lesson_stats.last_cited = Some(entry.ts);
                }
            }
        }

        stats
    }

    async fn check_promotion(
        &self,
        lesson: &Lesson,
        stats: &HashMap<LessonId, LessonStats>,
    ) -> Result<Option<TierTransition>> {
        let lesson_stats = stats.get(&lesson.id).cloned().unwrap_or_default();

        match lesson.tier {
            Tier::T0 => {
                // T0 -> T1: Any successful task completion promotes to T1
                // This is typically handled during task completion, not in tick()
                Ok(None)
            }
            Tier::T1 => {
                // T1 -> T2: Cited and contributed to success at least once
                if lesson_stats.citations >= self.config.min_citations_t1_t2
                    && lesson_stats.success_rate() >= self.config.min_success_rate_t1_t2
                {
                    let mut lesson = lesson.clone();
                    self.store.move_to_tier(&mut lesson, Tier::T2).await?;

                    return Ok(Some(TierTransition {
                        lesson_id: lesson.id.clone(),
                        from_tier: Tier::T1,
                        to_tier: Tier::T2,
                        reason: format!(
                            "Cited {} times with {:.0}% success rate",
                            lesson_stats.citations,
                            lesson_stats.success_rate() * 100.0
                        ),
                        timestamp: Utc::now(),
                    }));
                }
                Ok(None)
            }
            Tier::T2 => {
                // T2 -> T3: Requires human review in Foundry (not auto-promoted)
                // In foundry mode, this would queue for review
                #[cfg(feature = "foundry")]
                {
                    // Could run counterfactual A/B and queue for review
                }
                Ok(None)
            }
            Tier::T3 | Tier::T4 => {
                // T3 and T4 require human intervention
                Ok(None)
            }
        }
    }

    async fn check_demotion(
        &self,
        lesson: &Lesson,
        stats: &HashMap<LessonId, LessonStats>,
    ) -> Result<Option<TierTransition>> {
        let lesson_stats = stats.get(&lesson.id).cloned().unwrap_or_default();

        match lesson.tier {
            Tier::T0 | Tier::T4 => {
                // T0 lessons are discarded on failure (not demoted)
                // T4 lessons require manual rollback
                Ok(None)
            }
            Tier::T1 => {
                // T1 -> discard: Uncited for 30 days AND failures > 0
                if lesson_stats.citations == 0 {
                    let inactive_days = lesson_stats
                        .last_cited
                        .map(|t| (Utc::now() - t).num_days())
                        .unwrap_or_else(|| (Utc::now() - lesson.created).num_days());

                    if inactive_days >= self.config.demotion_inactive_days && lesson_stats.failures > 0 {
                        self.store.delete(lesson).await?;

                        return Ok(Some(TierTransition {
                            lesson_id: lesson.id.clone(),
                            from_tier: Tier::T1,
                            to_tier: Tier::T0, // Using T0 to indicate "discarded"
                            reason: format!(
                                "Inactive for {} days with {} failures",
                                inactive_days, lesson_stats.failures
                            ),
                            timestamp: Utc::now(),
                        }));
                    }
                }
                Ok(None)
            }
            Tier::T2 => {
                // T2 -> T1: Success rate < 50% over >= 5 citations
                if lesson_stats.citations >= self.config.demotion_min_citations
                    && lesson_stats.success_rate() < self.config.demotion_success_rate_threshold
                {
                    let mut lesson = lesson.clone();
                    self.store.move_to_tier(&mut lesson, Tier::T1).await?;

                    return Ok(Some(TierTransition {
                        lesson_id: lesson.id.clone(),
                        from_tier: Tier::T2,
                        to_tier: Tier::T1,
                        reason: format!(
                            "Success rate {:.0}% < {:.0}% over {} citations",
                            lesson_stats.success_rate() * 100.0,
                            self.config.demotion_success_rate_threshold * 100.0,
                            lesson_stats.citations
                        ),
                        timestamp: Utc::now(),
                    }));
                }
                Ok(None)
            }
            Tier::T3 => {
                // T3 -> T2: Recurring demotion signals
                // This requires more sophisticated analysis
                Ok(None)
            }
        }
    }

    /// Promote a specific lesson (manual promotion)
    pub async fn promote(&self, lesson_id: &LessonId, to_tier: Tier) -> Result<TierTransition> {
        let mut lesson = self.store.load(lesson_id).await?;
        let from_tier = lesson.tier;

        // Validate the transition
        if !self.is_valid_promotion(from_tier, to_tier) {
            return Err(PromotionError::InvalidTransition {
                from: from_tier,
                to: to_tier,
            });
        }

        self.store.move_to_tier(&mut lesson, to_tier).await?;

        Ok(TierTransition {
            lesson_id: lesson_id.clone(),
            from_tier,
            to_tier,
            reason: "Manual promotion".to_string(),
            timestamp: Utc::now(),
        })
    }

    /// Demote a specific lesson (manual demotion)
    pub async fn demote(&self, lesson_id: &LessonId, to_tier: Tier) -> Result<TierTransition> {
        let mut lesson = self.store.load(lesson_id).await?;
        let from_tier = lesson.tier;

        // Validate the transition
        if !self.is_valid_demotion(from_tier, to_tier) {
            return Err(PromotionError::InvalidTransition {
                from: from_tier,
                to: to_tier,
            });
        }

        self.store.move_to_tier(&mut lesson, to_tier).await?;

        Ok(TierTransition {
            lesson_id: lesson_id.clone(),
            from_tier,
            to_tier,
            reason: "Manual demotion".to_string(),
            timestamp: Utc::now(),
        })
    }

    fn is_valid_promotion(&self, from: Tier, to: Tier) -> bool {
        matches!(
            (from, to),
            (Tier::T0, Tier::T1)
                | (Tier::T1, Tier::T2)
                | (Tier::T2, Tier::T3)
                | (Tier::T3, Tier::T4)
        )
    }

    fn is_valid_demotion(&self, from: Tier, to: Tier) -> bool {
        matches!(
            (from, to),
            (Tier::T2, Tier::T1) | (Tier::T3, Tier::T2) | (Tier::T4, Tier::T3)
        )
    }
}

#[derive(Debug, Clone, Default)]
struct LessonStats {
    citations: u64,
    successes: u64,
    failures: u64,
    last_cited: Option<chrono::DateTime<chrono::Utc>>,
}

impl LessonStats {
    fn success_rate(&self) -> f64 {
        if self.citations == 0 {
            0.0
        } else {
            self.successes as f64 / self.citations as f64
        }
    }
}
