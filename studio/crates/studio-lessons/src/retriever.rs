use crate::error::Result;
use crate::ledger::LedgerWriter;
use crate::lesson::{Lesson, LessonId, Tier};
use crate::store::LessonStore;
use std::collections::HashSet;

/// Query parameters for lesson retrieval
#[derive(Debug, Clone, Default)]
pub struct RetrievalQuery {
    /// Tags to match against
    pub tags: Vec<String>,
    /// Phases to match against
    pub phases: Vec<String>,
    /// Free-form query text
    pub query_text: Option<String>,
    /// Minimum tier to include
    pub min_tier: Option<Tier>,
    /// Maximum number of results
    pub limit: usize,
}

impl RetrievalQuery {
    pub fn new() -> Self {
        Self {
            limit: 3, // Default top-K
            ..Default::default()
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_phases(mut self, phases: Vec<String>) -> Self {
        self.phases = phases;
        self
    }

    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query_text = Some(query.into());
        self
    }

    pub fn with_min_tier(mut self, tier: Tier) -> Self {
        self.min_tier = Some(tier);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// Result of retrieving a lesson
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub lesson: Lesson,
    pub score: f64,
    pub match_reasons: Vec<String>,
}

/// Retrieves and ranks lessons based on relevance
pub struct Retriever {
    store: LessonStore,
    ledger: LedgerWriter,
}

impl Retriever {
    pub fn new(store: LessonStore, ledger: LedgerWriter) -> Self {
        Self { store, ledger }
    }

    /// Retrieve lessons matching the query, ranked by relevance
    ///
    /// Ranking formula (without embeddings):
    /// score = tag_overlap * 0.45 + tier_weight * 0.35 + recent_success_rate * 0.20
    pub async fn retrieve(&self, query: &RetrievalQuery) -> Result<Vec<RetrievalResult>> {
        let lessons = self.store.list_all().await?;

        let mut results: Vec<RetrievalResult> = lessons
            .into_iter()
            .filter(|l| {
                if let Some(min_tier) = query.min_tier {
                    l.tier >= min_tier
                } else {
                    true
                }
            })
            .map(|lesson| {
                let (score, reasons) = self.score_lesson(&lesson, query);
                RetrievalResult {
                    lesson,
                    score,
                    match_reasons: reasons,
                }
            })
            .filter(|r| r.score > 0.0)
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Limit results
        results.truncate(query.limit);

        Ok(results)
    }

    fn score_lesson(&self, lesson: &Lesson, query: &RetrievalQuery) -> (f64, Vec<String>) {
        let mut score = 0.0;
        let mut reasons = Vec::new();

        // Tag overlap (0.45 weight)
        let tag_overlap = self.calculate_tag_overlap(&lesson.tags, &query.tags);
        if tag_overlap > 0.0 {
            score += tag_overlap * 0.45;
            reasons.push(format!("tag_overlap: {:.2}", tag_overlap));
        }

        // Phase overlap
        let phase_overlap = self.calculate_phase_overlap(&lesson.phases, &query.phases);
        if phase_overlap > 0.0 {
            score += phase_overlap * 0.15;
            reasons.push(format!("phase_overlap: {:.2}", phase_overlap));
        }

        // Tier weight (0.35 weight)
        let tier_weight = lesson.tier.weight();
        score += tier_weight * 0.35;
        reasons.push(format!("tier: {:?} ({:.2})", lesson.tier, tier_weight));

        // Recent success rate (0.20 weight)
        let success_rate = lesson.success_rate();
        if lesson.citations > 0 {
            score += success_rate * 0.20;
            reasons.push(format!(
                "success_rate: {:.2} ({}/{})",
                success_rate, lesson.successes, lesson.citations
            ));
        }

        (score, reasons)
    }

    fn calculate_tag_overlap(&self, lesson_tags: &[String], query_tags: &[String]) -> f64 {
        if query_tags.is_empty() {
            return 0.0;
        }

        let lesson_set: HashSet<&str> = lesson_tags.iter().map(|s| s.as_str()).collect();
        let query_set: HashSet<&str> = query_tags.iter().map(|s| s.as_str()).collect();

        let intersection = lesson_set.intersection(&query_set).count();
        intersection as f64 / query_tags.len() as f64
    }

    fn calculate_phase_overlap(&self, lesson_phases: &[String], query_phases: &[String]) -> f64 {
        if query_phases.is_empty() || lesson_phases.is_empty() {
            return 0.0;
        }

        let lesson_set: HashSet<&str> = lesson_phases.iter().map(|s| s.as_str()).collect();
        let query_set: HashSet<&str> = query_phases.iter().map(|s| s.as_str()).collect();

        let intersection = lesson_set.intersection(&query_set).count();
        if intersection > 0 {
            1.0
        } else {
            0.0
        }
    }

    /// Detect conflicting lessons among results
    pub fn detect_conflicts(results: &[RetrievalResult]) -> Vec<(LessonId, LessonId, String)> {
        let mut conflicts = Vec::new();

        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                let a = &results[i].lesson;
                let b = &results[j].lesson;

                // Check for opposing heuristics
                if let (
                    crate::lesson::LessonKind::Heuristic { prefer: a_prefer, avoid: a_avoid, .. },
                    crate::lesson::LessonKind::Heuristic { prefer: b_prefer, avoid: b_avoid, .. },
                ) = (&a.kind, &b.kind)
                {
                    // If one prefers what the other avoids
                    if Some(a_prefer) == b_avoid.as_ref() || Some(b_prefer) == a_avoid.as_ref() {
                        conflicts.push((
                            a.id.clone(),
                            b.id.clone(),
                            "Opposing heuristics".to_string(),
                        ));
                    }
                }

                // Check for anti-pattern vs positive recommendation
                if let (
                    crate::lesson::LessonKind::AntiPattern { pattern, .. },
                    crate::lesson::LessonKind::Heuristic { prefer, .. },
                ) = (&a.kind, &b.kind)
                {
                    if pattern.to_lowercase().contains(&prefer.to_lowercase())
                        || prefer.to_lowercase().contains(&pattern.to_lowercase())
                    {
                        conflicts.push((
                            a.id.clone(),
                            b.id.clone(),
                            "Anti-pattern vs recommendation".to_string(),
                        ));
                    }
                }
            }
        }

        conflicts
    }

    /// Format retrieved lessons as advisory context for the agent
    pub fn format_advisory(results: &[RetrievalResult]) -> String {
        if results.is_empty() {
            return String::new();
        }

        let mut output = String::from(
            "=== Staged guidance (advisory; cite by id if you apply one) ===\n",
        );

        for result in results {
            let lesson = &result.lesson;
            let domain = lesson.tags.first().map(|s| s.as_str()).unwrap_or("general");
            output.push_str(&format!(
                "[{} / {:?} / domain: {}] {}\n",
                lesson.id, lesson.tier, domain, lesson.title
            ));
            output.push_str(&format!("{}\n\n", lesson.body));
        }

        // Check for conflicts
        let conflicts = Self::detect_conflicts(results);
        if !conflicts.is_empty() {
            output.push_str("=== Conflicts detected ===\n");
            for (a, b, reason) in &conflicts {
                output.push_str(&format!(
                    "Lessons {} and {} disagree ({}) — pick one and explain why.\n",
                    a, b, reason
                ));
            }
        }

        output.push_str("=== End advisory ===\n");
        output
    }
}
