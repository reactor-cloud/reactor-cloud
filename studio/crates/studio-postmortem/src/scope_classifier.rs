use crate::error::{PostmortemError, Result};
use studio_lessons::{Lesson, Scope};
use tracing::{debug, info};

/// Classifies the scope of a lesson (project vs global)
pub struct ScopeClassifier;

impl ScopeClassifier {
    pub fn new() -> Self {
        Self
    }

    /// Classify the scope of a lesson based on its content
    pub async fn classify(&self, lesson: &Lesson) -> Result<Scope> {
        let mut global_score = 0i32;

        // Check tags for generic vs project-specific indicators
        global_score += self.score_tags(&lesson.tags);

        // Check body content for project-specific patterns
        global_score += self.score_body(&lesson.body);

        // Check title for project-specific patterns
        global_score += self.score_title(&lesson.title);

        debug!(
            "Scope classification for {}: score = {} -> {:?}",
            lesson.id,
            global_score,
            if global_score > 0 {
                Scope::GlobalCandidate
            } else {
                Scope::Project
            }
        );

        Ok(if global_score > 0 {
            Scope::GlobalCandidate
        } else {
            Scope::Project
        })
    }

    fn score_tags(&self, tags: &[String]) -> i32 {
        let mut score = 0i32;

        let generic_tags = [
            "typescript",
            "javascript",
            "rust",
            "env",
            "configuration",
            "import",
            "module",
            "dependency",
            "auth",
            "database",
            "api",
            "http",
            "error-handling",
            "testing",
            "reactor.cloud",
            "reactor",
        ];

        let project_specific_patterns = [
            "src/",
            "app/",
            "components/",
            "pages/",
            ".env.",
        ];

        for tag in tags {
            let lower = tag.to_lowercase();

            // Generic tags increase global score
            if generic_tags.iter().any(|g| lower.contains(g)) {
                score += 1;
            }

            // Project-specific patterns decrease score
            if project_specific_patterns.iter().any(|p| lower.contains(p)) {
                score -= 2;
            }
        }

        score
    }

    fn score_body(&self, body: &str) -> i32 {
        let mut score = 0i32;
        let lower = body.to_lowercase();

        // Project-specific indicators (paths, file names, etc.)
        let project_patterns = [
            "/users/",
            "/home/",
            "c:\\",
            "d:\\",
            ".env.local",
            ".env.development",
            "localhost:",
            "127.0.0.1",
        ];

        for pattern in project_patterns {
            if lower.contains(pattern) {
                score -= 2;
            }
        }

        // Generic concept indicators
        let generic_patterns = [
            "in general",
            "always",
            "typically",
            "best practice",
            "recommended",
            "standard",
            "common pattern",
            "reactor.cloud",
            "when deploying",
            "when building",
        ];

        for pattern in generic_patterns {
            if lower.contains(pattern) {
                score += 1;
            }
        }

        // Check for specific project names or custom identifiers
        // These would be provided by config in a real implementation
        let project_specific_names = [
            "my-app",
            "my-project",
            "test-project",
        ];

        for name in project_specific_names {
            if lower.contains(name) {
                score -= 3;
            }
        }

        score
    }

    fn score_title(&self, title: &str) -> i32 {
        let mut score = 0i32;
        let lower = title.to_lowercase();

        // Generic titles
        let generic_patterns = [
            "always",
            "avoid",
            "prefer",
            "handle",
            "when",
            "error",
            "pattern",
        ];

        for pattern in generic_patterns {
            if lower.contains(pattern) {
                score += 1;
            }
        }

        // Project-specific title patterns
        if lower.contains("fix ") && lower.contains(" in ") {
            score -= 1;
        }

        score
    }

    /// Check if a lesson could be rewritten without project-specific terms
    pub fn can_generalize(&self, lesson: &Lesson) -> bool {
        let body_lower = lesson.body.to_lowercase();

        // If it contains absolute paths, it's probably project-specific
        if body_lower.contains("/users/") || body_lower.contains("/home/") {
            return false;
        }

        // If it references specific config files with local paths
        if body_lower.contains(".env.local") && body_lower.contains("must be set to") {
            return false;
        }

        true
    }
}

impl Default for ScopeClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_lessons::{LessonKind, Origin};

    #[tokio::test]
    async fn test_classify_generic_lesson() {
        let classifier = ScopeClassifier::new();
        
        let lesson = Lesson::new(
            LessonKind::Heuristic {
                when: "environment variable is undefined".to_string(),
                prefer: "Validate env vars at startup".to_string(),
                avoid: None,
            },
            "Always validate environment variables".to_string(),
            "Environment variables should be validated at application startup. \
            This is a best practice for reactor.cloud applications.".to_string(),
            Origin::Postmortem,
        )
        .with_tags(vec!["env".to_string(), "configuration".to_string()]);

        let scope = classifier.classify(&lesson).await.unwrap();
        assert_eq!(scope, Scope::GlobalCandidate);
    }

    #[tokio::test]
    async fn test_classify_project_specific_lesson() {
        let classifier = ScopeClassifier::new();
        
        let lesson = Lesson::new(
            LessonKind::Heuristic {
                when: "deploying my-app".to_string(),
                prefer: "Use the settings in /users/dev/my-app/.env.local".to_string(),
                avoid: None,
            },
            "Fix deployment for my-app".to_string(),
            "When deploying my-app, make sure /users/dev/my-app/.env.local has the correct API key.".to_string(),
            Origin::Postmortem,
        );

        let scope = classifier.classify(&lesson).await.unwrap();
        assert_eq!(scope, Scope::Project);
    }
}
