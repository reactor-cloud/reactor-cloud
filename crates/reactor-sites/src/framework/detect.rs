//! Framework detection logic.

use crate::Framework;
use std::path::Path;

/// Detect the framework used by a project.
pub fn detect_framework(project_dir: &Path) -> Option<Framework> {
    let package_json = project_dir.join("package.json");

    if !package_json.exists() {
        if has_static_files(project_dir) {
            return Some(Framework::Static);
        }
        return None;
    }

    let content = std::fs::read_to_string(&package_json).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;

    let deps = pkg.get("dependencies").and_then(|d| d.as_object());
    let dev_deps = pkg.get("devDependencies").and_then(|d| d.as_object());

    let has_dep = |name: &str| {
        deps.map(|d| d.contains_key(name)).unwrap_or(false)
            || dev_deps.map(|d| d.contains_key(name)).unwrap_or(false)
    };

    if has_dep("next") {
        return Some(Framework::Nextjs);
    }

    if has_dep("hono") {
        return Some(Framework::Hono);
    }

    let has_build_script = pkg
        .get("scripts")
        .and_then(|s| s.get("build"))
        .is_some();

    if !has_build_script && has_static_files(project_dir) {
        return Some(Framework::Static);
    }

    None
}

/// Check if a directory contains static web files.
fn has_static_files(dir: &Path) -> bool {
    if dir.join("index.html").exists() {
        return true;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    match ext.to_str() {
                        Some("html" | "htm" | "css" | "js") => return true,
                        _ => {}
                    }
                }
            }
        }
    }

    false
}
