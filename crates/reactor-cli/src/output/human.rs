//! Human-readable output formatting.

use crate::error::CliError;
use console::{style, Term};

/// Print a success message.
pub fn print_success(message: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{} {}", style("✓").green().bold(), message));
}

/// Print an error message.
pub fn print_error(error: &CliError) {
    let term = Term::stderr();
    let _ = term.write_line(&format!(
        "{} {}",
        style("error:").red().bold(),
        error
    ));
    if let Some(hint) = error.hint() {
        let _ = term.write_line(&format!("  {} {}", style("hint:").yellow(), hint));
    }
}

/// Print a warning message.
pub fn print_warning(message: &str) {
    let term = Term::stderr();
    let _ = term.write_line(&format!(
        "{} {}",
        style("warning:").yellow().bold(),
        message
    ));
}

/// Print an info message.
pub fn print_info(message: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{} {}", style("→").cyan(), message));
}

/// Print a table with headers and rows.
pub fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        println!("(no items)");
        return;
    }

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    // Print header
    let header_line: String = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:width$}", h.to_uppercase(), width = widths[i]))
        .collect::<Vec<_>>()
        .join("  ");
    println!("{}", style(header_line).bold());

    // Print separator
    let sep: String = widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join("  ");
    println!("{}", style(sep).dim());

    // Print rows
    for row in rows {
        let line: String = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let width = widths.get(i).copied().unwrap_or(cell.len());
                format!("{:width$}", cell, width = width)
            })
            .collect::<Vec<_>>()
            .join("  ");
        println!("{}", line);
    }
}

/// Print a key-value pair.
pub fn print_kv(key: &str, value: &str) {
    println!("{}: {}", style(key).bold(), value);
}

/// Print a section header.
pub fn print_section(title: &str) {
    println!();
    println!("{}", style(title).bold().underlined());
}

/// Print a bullet point.
pub fn print_bullet(message: &str) {
    println!("  • {}", message);
}

/// Create a progress bar.
pub fn progress_bar(len: u64, message: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(len);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(message.to_string());
    pb
}

/// Create a spinner.
pub fn spinner(message: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}
