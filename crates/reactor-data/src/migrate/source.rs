//! Migration source abstraction.
//!
//! Supports filesystem discovery of SQL migration files.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// A discovered migration file.
#[derive(Debug, Clone)]
pub struct MigrationFile {
    /// Migration name (filename without extension).
    pub name: String,
    /// Full path to the file.
    pub path: PathBuf,
    /// Raw SQL content.
    pub content: String,
    /// SHA-256 hash of the content.
    pub checksum: String,
}

impl MigrationFile {
    /// Compute the SHA-256 checksum of the given content.
    pub fn compute_checksum(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }
}

/// Trait for migration sources.
pub trait MigrationSource: Send + Sync {
    /// Discover all migration files from this source.
    fn discover(&self) -> Result<Vec<MigrationFile>, std::io::Error>;
}

/// Filesystem-based migration source.
#[derive(Debug, Clone)]
pub struct FilesystemSource {
    directory: PathBuf,
}

impl FilesystemSource {
    /// Create a new filesystem source from the given directory.
    pub fn new<P: AsRef<Path>>(directory: P) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
        }
    }

    /// Get the directory path.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

impl MigrationSource for FilesystemSource {
    fn discover(&self) -> Result<Vec<MigrationFile>, std::io::Error> {
        let mut files = Vec::new();

        if !self.directory.exists() {
            return Ok(files);
        }

        let mut entries: Vec<_> = std::fs::read_dir(&self.directory)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "sql")
                    .unwrap_or(false)
            })
            .collect();

        // Sort lexicographically by filename
        entries.sort_by_key(|a| a.file_name());

        for entry in entries {
            let path = entry.path();
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let content = std::fs::read_to_string(&path)?;
            let checksum = MigrationFile::compute_checksum(&content);

            files.push(MigrationFile {
                name,
                path,
                content,
                checksum,
            });
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_checksum() {
        let content = "CREATE TABLE test (id INT);";
        let checksum = MigrationFile::compute_checksum(content);
        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64); // SHA-256 hex

        // Same content = same checksum
        let checksum2 = MigrationFile::compute_checksum(content);
        assert_eq!(checksum, checksum2);

        // Different content = different checksum
        let checksum3 = MigrationFile::compute_checksum("something else");
        assert_ne!(checksum, checksum3);
    }

    #[test]
    fn test_discover_empty_dir() {
        let dir = tempdir().unwrap();
        let source = FilesystemSource::new(dir.path());
        let files = source.discover().unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_sql_files() {
        let dir = tempdir().unwrap();

        // Create some SQL files
        let mut f1 = std::fs::File::create(dir.path().join("001_init.sql")).unwrap();
        writeln!(f1, "CREATE TABLE users (id INT);").unwrap();

        let mut f2 = std::fs::File::create(dir.path().join("002_posts.sql")).unwrap();
        writeln!(f2, "CREATE TABLE posts (id INT);").unwrap();

        // Create a non-SQL file that should be ignored
        let mut f3 = std::fs::File::create(dir.path().join("readme.txt")).unwrap();
        writeln!(f3, "This is not SQL").unwrap();

        let source = FilesystemSource::new(dir.path());
        let files = source.discover().unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "001_init");
        assert_eq!(files[1].name, "002_posts");
    }

    #[test]
    fn test_discover_lexicographic_order() {
        let dir = tempdir().unwrap();

        // Create files out of order
        std::fs::write(dir.path().join("003_third.sql"), "SELECT 3;").unwrap();
        std::fs::write(dir.path().join("001_first.sql"), "SELECT 1;").unwrap();
        std::fs::write(dir.path().join("002_second.sql"), "SELECT 2;").unwrap();

        let source = FilesystemSource::new(dir.path());
        let files = source.discover().unwrap();

        assert_eq!(files.len(), 3);
        assert_eq!(files[0].name, "001_first");
        assert_eq!(files[1].name, "002_second");
        assert_eq!(files[2].name, "003_third");
    }
}
