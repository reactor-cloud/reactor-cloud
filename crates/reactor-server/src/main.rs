//! Reactor Server binary entrypoint.

use anyhow::Result;
use reactor_server::ReactorConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle subcommands
    if args.len() > 1 {
        match args[1].as_str() {
            "migrate" => {
                let config = ReactorConfig::load()?;
                return reactor_server::migrate_only(config).await;
            }
            "doctor" => {
                let config = ReactorConfig::load()?;
                return reactor_server::doctor_only(config).await;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" | "-V" => {
                println!("reactor-server {}", reactor_server::VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    // Default: run the server
    let config = ReactorConfig::load()?;
    reactor_server::run(config).await
}

fn print_help() {
    println!(
        r#"reactor-server {}

Unified Reactor.cloud server for G1/G2 topologies.

USAGE:
    reactor-server [COMMAND]

COMMANDS:
    (none)    Start the HTTP server
    migrate   Run all capability migrations and exit
    doctor    Run health checks and exit

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

CONFIGURATION:
    The server reads configuration from:
    1. Reactor.toml (or --config <path>)
    2. REACTOR_* environment variables
    3. Per-capability REACTOR_AUTH_*, REACTOR_DATA_*, etc.

See docs/reactor-server.design.md for full documentation."#,
        reactor_server::VERSION
    );
}
