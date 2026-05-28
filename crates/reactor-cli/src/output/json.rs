//! JSON output formatting.
//!
//! All JSON output follows the envelope format:
//! - Success: `{ "ok": true, "data": <payload> }`
//! - Error: `{ "ok": false, "error": { "code": "...", "message": "...", "hint": "..." } }`

use crate::error::{CliError, CliResult};
use serde::Serialize;
use std::io::Write;

/// Success envelope.
#[derive(Debug, Serialize)]
pub struct SuccessEnvelope<T> {
    pub ok: bool,
    pub data: T,
}

/// Error envelope.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorDetail,
}

/// Error detail.
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Write a successful result to stdout.
pub fn write_success<T: Serialize>(data: &T) -> CliResult<()> {
    let envelope = SuccessEnvelope { ok: true, data };
    let json = serde_json::to_string_pretty(&envelope)?;
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", json)?;
    Ok(())
}

/// Write an error to stderr.
pub fn write_error(error: &CliError) -> CliResult<()> {
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorDetail {
            code: error.code().to_string(),
            message: error.to_string(),
            hint: error.hint().map(|s| s.to_string()),
        },
    };
    let json = serde_json::to_string_pretty(&envelope)?;
    let mut stderr = std::io::stderr().lock();
    writeln!(stderr, "{}", json)?;
    Ok(())
}

/// Write raw JSON value to stdout.
pub fn write_raw(value: &serde_json::Value) -> CliResult<()> {
    let json = serde_json::to_string_pretty(value)?;
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_envelope() {
        let envelope = SuccessEnvelope {
            ok: true,
            data: serde_json::json!({ "version": "0.1.0" }),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains(r#""ok":true"#));
        assert!(json.contains(r#""version":"0.1.0""#));
    }

    #[test]
    fn test_error_envelope() {
        let envelope = ErrorEnvelope {
            ok: false,
            error: ErrorDetail {
                code: "TEST_ERROR".to_string(),
                message: "something went wrong".to_string(),
                hint: Some("try again".to_string()),
            },
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains(r#""ok":false"#));
        assert!(json.contains(r#""code":"TEST_ERROR""#));
        assert!(json.contains(r#""hint":"try again""#));
    }

    #[test]
    fn test_error_envelope_no_hint() {
        let envelope = ErrorEnvelope {
            ok: false,
            error: ErrorDetail {
                code: "TEST_ERROR".to_string(),
                message: "something went wrong".to_string(),
                hint: None,
            },
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(!json.contains("hint"));
    }
}
