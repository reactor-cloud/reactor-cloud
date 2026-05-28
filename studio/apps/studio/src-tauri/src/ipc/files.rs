use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("IO error: {0}")]
    Io(String),
}

impl Serialize for FileError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for FileError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(e.to_string()),
            std::io::ErrorKind::PermissionDenied => FileError::PermissionDenied(e.to_string()),
            _ => FileError::Io(e.to_string()),
        }
    }
}

#[tauri::command]
pub async fn file_read(path: String) -> Result<String, FileError> {
    let content = fs::read_to_string(&path)?;
    Ok(content)
}

#[tauri::command]
pub async fn file_write(path: String, contents: String) -> Result<(), FileError> {
    // Ensure parent directory exists
    if let Some(parent) = PathBuf::from(&path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, contents)?;
    Ok(())
}

#[tauri::command]
pub async fn file_list(path: String) -> Result<Vec<FileEntry>, FileError> {
    let mut entries = Vec::new();

    let read_dir = fs::read_dir(&path)?;

    for entry in read_dir {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files (starting with .)
        if name.starts_with('.') {
            continue;
        }

        entries.push(FileEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            is_file: metadata.is_file(),
            size: if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            },
        });
    }

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(entries)
}
