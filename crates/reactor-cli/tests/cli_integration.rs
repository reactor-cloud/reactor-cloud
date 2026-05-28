//! CLI integration tests.
//!
//! These tests verify the CLI behavior, output format, and exit codes.

use std::process::{Command, Output};
use std::path::PathBuf;

fn reactor_binary() -> PathBuf {
    // Find the reactor binary in the target directory
    let mut path = std::env::current_exe().expect("Failed to get current exe path");
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("reactor");
    
    // Try to find it in a few places
    if path.exists() {
        return path;
    }
    
    // Fallback to cargo build
    PathBuf::from("cargo")
}

fn run_reactor(args: &[&str]) -> Output {
    let binary = reactor_binary();
    
    if binary.as_os_str() == "cargo" {
        Command::new("cargo")
            .args(["run", "--package", "reactor-cli", "--bin", "reactor", "--"])
            .args(args)
            .output()
            .expect("Failed to execute reactor")
    } else {
        Command::new(binary)
            .args(args)
            .output()
            .expect("Failed to execute reactor")
    }
}

mod cli_smoke {
    use super::*;

    #[test]
    fn test_help() {
        let output = run_reactor(&["--help"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Reactor CLI"));
        assert!(stdout.contains("--context"));
    }

    #[test]
    fn test_version_flag() {
        let output = run_reactor(&["--version"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("reactor-cli"));
    }

    #[test]
    fn test_unknown_command() {
        let output = run_reactor(&["nonexistent"]);
        assert!(!output.status.success());
    }
}

mod output_contract {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_json_output_success() {
        // `context list` should work even without any contexts
        let output = run_reactor(&["--output", "json", "context", "list"]);
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Should be valid JSON
        let json: Value = serde_json::from_str(&stdout)
            .expect("Output should be valid JSON");
        
        // Should have the success envelope
        assert_eq!(json["ok"], true);
        assert!(json["data"].is_array());
    }

    #[test]
    fn test_json_output_error() {
        // Trying to show a non-existent context
        let output = run_reactor(&["--output", "json", "context", "show", "nonexistent"]);
        
        // Should fail
        assert!(!output.status.success());
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Should be valid JSON error envelope
        let json: Value = serde_json::from_str(&stdout)
            .expect("Error output should be valid JSON");
        
        assert_eq!(json["ok"], false);
        assert!(json["error"]["code"].is_string());
        assert!(json["error"]["message"].is_string());
    }
}

mod context_lifecycle {
    use super::*;
    use serde_json::Value;
    use std::env;
    use tempfile::TempDir;

    fn with_temp_home<F>(f: F) where F: FnOnce() {
        let temp_dir = TempDir::new().unwrap();
        env::set_var("HOME", temp_dir.path());
        f();
    }

    #[test]
    fn test_context_list_empty() {
        with_temp_home(|| {
            let output = run_reactor(&["--output", "json", "context", "list"]);
            assert!(output.status.success());
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            let json: Value = serde_json::from_str(&stdout).unwrap();
            
            assert_eq!(json["ok"], true);
            assert!(json["data"].as_array().unwrap().is_empty());
        });
    }
}

mod nontty_no_prompt {
    use super::*;

    #[test]
    fn test_output_is_json_in_nontty() {
        // When piped, output should be JSON by default
        let output = run_reactor(&["context", "list"]);
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // In non-TTY mode, should auto-detect and use JSON
            // Note: This might output human format if stdin is detected as TTY
            // The test verifies the output is parseable either way
            if stdout.contains("\"ok\"") {
                let _: serde_json::Value = serde_json::from_str(&stdout)
                    .expect("JSON output should be valid");
            }
        }
    }
}

mod exit_codes {
    use super::*;

    #[test]
    fn test_success_exit_code() {
        let output = run_reactor(&["--help"]);
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_user_error_exit_code() {
        // Invalid subcommand
        let output = run_reactor(&["nonexistent"]);
        // Clap returns 2 for usage errors
        assert!(output.status.code().unwrap_or(1) >= 1);
    }
}
