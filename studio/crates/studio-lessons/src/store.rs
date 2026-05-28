use crate::error::{LessonError, Result};
use crate::lesson::{Lesson, LessonId, Tier};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// On-disk storage for lessons, organized by tier
pub struct LessonStore {
    base_path: PathBuf,
}

impl LessonStore {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Initialize the store directories
    pub async fn init(&self) -> Result<()> {
        for tier in [Tier::T0, Tier::T1, Tier::T2, Tier::T3, Tier::T4] {
            let dir = self.tier_dir(tier);
            fs::create_dir_all(&dir).await?;
            debug!("Created lesson directory: {:?}", dir);
        }
        Ok(())
    }

    fn tier_dir(&self, tier: Tier) -> PathBuf {
        self.base_path.join(tier.storage_dir())
    }

    fn lesson_path(&self, lesson: &Lesson) -> PathBuf {
        self.tier_dir(lesson.tier).join(format!("{}.yaml", lesson.id.0))
    }

    fn lesson_path_by_id(&self, tier: Tier, id: &LessonId) -> PathBuf {
        self.tier_dir(tier).join(format!("{}.yaml", id.0))
    }

    /// Save a lesson to disk
    pub async fn save(&self, lesson: &Lesson) -> Result<()> {
        let path = self.lesson_path(lesson);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let yaml = serde_yaml::to_string(lesson)?;
        fs::write(&path, yaml).await?;
        info!("Saved lesson {} to {:?}", lesson.id, path);

        Ok(())
    }

    /// Load a lesson by ID, searching through all tiers
    pub async fn load(&self, id: &LessonId) -> Result<Lesson> {
        for tier in [Tier::T0, Tier::T1, Tier::T2, Tier::T3, Tier::T4] {
            let path = self.lesson_path_by_id(tier, id);
            if path.exists() {
                let content = fs::read_to_string(&path).await?;
                let lesson: Lesson = serde_yaml::from_str(&content)?;
                return Ok(lesson);
            }
        }

        Err(LessonError::NotFound(id.0.clone()))
    }

    /// Load a lesson from a specific tier
    pub async fn load_from_tier(&self, tier: Tier, id: &LessonId) -> Result<Lesson> {
        let path = self.lesson_path_by_id(tier, id);
        if !path.exists() {
            return Err(LessonError::NotFound(id.0.clone()));
        }

        let content = fs::read_to_string(&path).await?;
        let lesson: Lesson = serde_yaml::from_str(&content)?;
        Ok(lesson)
    }

    /// Delete a lesson
    pub async fn delete(&self, lesson: &Lesson) -> Result<()> {
        let path = self.lesson_path(lesson);
        if path.exists() {
            fs::remove_file(&path).await?;
            info!("Deleted lesson {} from {:?}", lesson.id, path);
        }
        Ok(())
    }

    /// Move a lesson to a different tier
    pub async fn move_to_tier(&self, lesson: &mut Lesson, new_tier: Tier) -> Result<()> {
        let old_path = self.lesson_path(lesson);
        lesson.tier = new_tier;
        lesson.updated = chrono::Utc::now();
        let new_path = self.lesson_path(lesson);

        // Save to new location first
        self.save(lesson).await?;

        // Then delete old location if different
        if old_path != new_path && old_path.exists() {
            fs::remove_file(&old_path).await?;
        }

        info!(
            "Moved lesson {} to tier {:?}",
            lesson.id, new_tier
        );
        Ok(())
    }

    /// List all lessons in a tier
    pub async fn list_tier(&self, tier: Tier) -> Result<Vec<Lesson>> {
        let dir = self.tier_dir(tier);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut lessons = Vec::new();
        let mut entries = fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                match fs::read_to_string(&path).await {
                    Ok(content) => match serde_yaml::from_str::<Lesson>(&content) {
                        Ok(lesson) => lessons.push(lesson),
                        Err(e) => warn!("Failed to parse lesson at {:?}: {}", path, e),
                    },
                    Err(e) => warn!("Failed to read lesson at {:?}: {}", path, e),
                }
            }
        }

        Ok(lessons)
    }

    /// List all lessons across all tiers
    pub async fn list_all(&self) -> Result<Vec<Lesson>> {
        let mut all = Vec::new();
        for tier in [Tier::T0, Tier::T1, Tier::T2, Tier::T3, Tier::T4] {
            all.extend(self.list_tier(tier).await?);
        }
        Ok(all)
    }

    /// Count lessons by tier
    pub async fn count_by_tier(&self) -> Result<std::collections::HashMap<Tier, usize>> {
        let mut counts = std::collections::HashMap::new();
        for tier in [Tier::T0, Tier::T1, Tier::T2, Tier::T3, Tier::T4] {
            counts.insert(tier, self.list_tier(tier).await?.len());
        }
        Ok(counts)
    }

    /// Stage a lesson (save at T1)
    pub async fn stage(&self, mut lesson: Lesson) -> Result<Lesson> {
        lesson.tier = Tier::T1;
        lesson.updated = chrono::Utc::now();
        self.save(&lesson).await?;
        Ok(lesson)
    }
}
