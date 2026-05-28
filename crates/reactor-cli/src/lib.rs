//! Reactor CLI library.
//!
//! This crate provides the core functionality for the `reactor` CLI binary.
//! The library entry point is `run(argv)` which returns an `ExitCode`.

pub mod bundle;
pub mod cli;
pub mod cloudflare;
pub mod commands;
pub mod context;
pub mod error;
pub mod output;
pub mod project;

use cli::{Cli, Commands};
use clap::Parser;
use error::{CliError, CliResult};
use output::{Output, OutputFormat};
use std::process::ExitCode;

/// Run the CLI with the given arguments.
///
/// This is the main entry point for the CLI, designed to be testable.
pub async fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            // Clap handles its own error output
            e.exit();
        }
    };

    let output_format = OutputFormat::resolve(cli.output);
    let output = Output::new(output_format);

    match run_command(&cli, &output).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            output.error(&e);
            e.exit_code().into()
        }
    }
}

/// Run the parsed command.
async fn run_command(cli: &Cli, output: &Output) -> CliResult<()> {
    match &cli.command {
        Commands::Version(_) => commands::version::run(cli, output).await,
        Commands::Doctor(_) => commands::doctor::run(cli, output).await,
        Commands::Init(args) => commands::init::run(cli, args, output).await,
        Commands::Project(args) => commands::project::run(cli, args, output).await,
        Commands::Context(args) => commands::context::run(cli, args, output).await,
        Commands::Login(args) => commands::login::run(cli, args, output).await,
        Commands::Logout(args) => commands::logout::run(cli, args, output).await,
        Commands::Whoami(_) => commands::whoami::run(cli, output).await,
        Commands::Migrate(args) => commands::migrate::run(cli, args, output).await,
        Commands::Build(args) => commands::build::run(cli, args, output).await,
        Commands::Deploy(args) => commands::deploy::run(cli, args, output).await,
        Commands::Functions(args) => commands::functions::run(cli, args, output).await,
        Commands::Sites(args) => commands::sites::run(cli, args, output).await,
        Commands::Jobs(args) => commands::jobs::run(cli, args, output).await,
        Commands::Data(args) => commands::data::run(cli, args, output).await,
        Commands::Ai(args) => commands::ai::run(cli, args, output).await,
        Commands::Auth(args) => commands::auth::run(cli, args, output).await,
        Commands::Vault(args) => commands::vault::run(cli, &args.command, output).await,
        Commands::Cloud(args) => commands::cloud::run(cli, args, output).await,
        Commands::Connect(args) => commands::connect::run(cli, args, output).await,
        Commands::Inspect(args) => commands::inspect::run(cli, args, output).await,
        Commands::Logs(args) => commands::logs::run(cli, args, output).await,
        Commands::Types(args) => commands::types::run(cli, args, output).await,

        #[cfg(feature = "dev")]
        Commands::Dev(args) => commands::dev::run(cli, args, output).await,
        #[cfg(feature = "dev")]
        Commands::Up(args) => commands::up::run(cli, args, output).await,
        #[cfg(feature = "dev")]
        Commands::Down(args) => commands::down::run(cli, args, output).await,
        #[cfg(feature = "dev")]
        Commands::Status(args) => commands::status::run(cli, args, output).await,
    }
}

/// CLI version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if running in interactive mode.
pub fn is_interactive() -> bool {
    output::is_tty() && output::is_stdin_tty()
}

/// Prompt for confirmation in interactive mode, or check --yes flag.
pub fn confirm(cli: &Cli, message: &str) -> CliResult<()> {
    if cli.yes {
        return Ok(());
    }

    if !is_interactive() {
        return Err(CliError::RequiresConfirmation);
    }

    // In interactive mode, prompt for confirmation
    use std::io::{self, Write};
    print!("{} [y/N] ", message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        Ok(())
    } else {
        Err(CliError::User("aborted".into()))
    }
}
