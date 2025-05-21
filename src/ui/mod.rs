use colored::*;

// Get terminal width (fallback to 80 if can't determine)
fn get_terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(width, _)| width.0 as usize)
        .unwrap_or(80)
}

// Print simplified header with just the title
pub fn print_header() {
    println!("{}", "🎭  Theater Commit".bright_blue().bold());
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
        let padding = width.saturating_sub(char_count) - 2;
        println!("  │ {}{} │", line, " ".repeat(padding));
    }

    println!("  └{}┘", line);
}

// Print a minimal completion message
pub fn print_completion(success: bool, duration_secs: f64) {
    if success {
        println!("Done {:?}s", duration_secs);
    } else {
        println!("Done with issues {:?}s", duration_secs);
    }
}
