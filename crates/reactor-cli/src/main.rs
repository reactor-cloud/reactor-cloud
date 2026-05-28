//! Reactor CLI entry point.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    // Set up panic hook to convert panics to exit codes
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };

        // In JSON mode, output a structured error
        if !console::Term::stdout().is_term() {
            let error = serde_json::json!({
                "ok": false,
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": format!("internal error: {}", message),
                    "hint": "This is a bug. Please report it."
                }
            });
            eprintln!("{}", serde_json::to_string_pretty(&error).unwrap_or_default());
        } else {
            eprintln!("error: internal error: {}", message);
            if let Some(location) = info.location() {
                eprintln!("  at {}:{}:{}", location.file(), location.line(), location.column());
            }
            eprintln!("  hint: This is a bug. Please report it.");
        }
    }));

    // Initialize tracing (only if verbose)
    if std::env::args().any(|a| a == "-v" || a == "--verbose") {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::DEBUG.into()),
            )
            .with_target(false)
            .init();
    }

    // Run the CLI
    reactor_cli::run(std::env::args_os()).await
}
