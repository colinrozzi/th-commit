use colored::*;
use std::io::{self, Write};

// Get terminal width (fallback to 80 if can't determine)
fn get_terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(width, _)| width.0 as usize)
        .unwrap_or(80)
}

// Print header with logo and title
pub fn print_header() {
    let width = get_terminal_width().min(100);
    let box_width = width.min(50);
    let line = "─".repeat(box_width - 2);

    println!("{}", format!("╭{}╮", line).bright_blue());
    println!(
        "{}",
        format!("│{}│", " ".repeat(box_width - 2)).bright_blue()
    );

    // Calculate proper padding for center alignment
    let title = "🎭  Theater Commit";
    let title_len = title.chars().count();
    let left_padding = (box_width - 2 - title_len) / 2;
    let right_padding = box_width - 2 - title_len - left_padding - 1;

    println!(
        "{}",
        format!(
            "│{}{}{}│",
            " ".repeat(left_padding),
            title,
            " ".repeat(right_padding)
        )
        .bright_blue()
    );

    println!(
        "{}",
        format!("│{}│", " ".repeat(box_width - 2)).bright_blue()
    );
    println!("{}", format!("╰{}╯", line).bright_blue());
}

// Print section title
pub fn print_section(title: &str) {
    println!();
    println!("{}", title.bold().underline());
}

// Print labeled item with optional color
pub fn print_item<S: AsRef<str>>(label: &str, value: S, color: Option<&str>) {
    let value = value.as_ref();
    let colored_value = match color {
        Some("success") => value.green(),
        Some("error") => value.red(),
        Some("warning") => value.yellow(),
        Some("info") => value.bright_blue(),
        Some("highlight") => value.cyan(),
        Some("dim") => value.dimmed(),
        _ => value.normal(),
    };

    println!("  {} {}", format!("{}:", label).bold(), colored_value);
}

// Print a status line with icon
pub fn print_status<S: AsRef<str>>(message: S, status_type: &str) {
    let message = message.as_ref();
    let (icon, colored_message) = match status_type {
        "success" => ("✅", message.green()),
        "error" => ("❌", message.red()),
        "warning" => ("⚠️", message.yellow()),
        "info" => ("ℹ️", message.normal()),
        "working" => ("⏳", message.normal()),
        "analyzing" => ("🔍", message.normal()),
        "robot" => ("🤖", message.normal()),
        "files" => ("📁", message.normal()),
        "message" => ("💬", message.normal()),
        "stats" => ("📊", message.normal()),
        _ => ("•", message.normal()),
    };

    println!("{} {}", icon, colored_message);
}

// Print a horizontal separator line
pub fn print_separator() {
    let width = get_terminal_width().min(100);
    println!("{}", "─".repeat(width.min(50)).dimmed());
}

// Print a framed box for commit messages
pub fn print_commit_message(message: &str) {
    let width = get_terminal_width().min(90) - 6;
    let line = "─".repeat(width);

    println!("  ┌{}┐", line);

    // Split message by lines and print each with padding
    for line in message.lines() {
        // Use unicode-aware character counting to get proper length
        let char_count = line.chars().count();
        let padding = width.saturating_sub(char_count);
        println!("  │ {}{} │", line, " ".repeat(padding));
    }

    println!("  └{}┘", line);
}

// Print error message
pub fn print_error(message: &str) {
    eprintln!("{} {}", "Error:".bold().red(), message);
}

// Print a completion summary with execution time
pub fn print_completion(success: bool, duration_secs: f64) {
    println!();
    if success {
        println!(
            "{} in {:.1}s",
            "Completed successfully".green(),
            duration_secs
        );
    } else {
        println!(
            "{} in {:.1}s",
            "Completed with issues".yellow(),
            duration_secs
        );
    }
}

// Clear current line and write new content
pub fn update_status(message: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    // Clear the current line
    write!(stdout, "\r\x1B[K")?;
    // Write the new status
    write!(stdout, "{}", message)?;
    stdout.flush()?;
    Ok(())
}
