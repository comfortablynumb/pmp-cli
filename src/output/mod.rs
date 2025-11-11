//! Beautiful terminal output styling for PMP CLI
//!
//! This module provides styled output functions inspired by Claude Code's aesthetic.

#![allow(dead_code)]

use owo_colors::OwoColorize;

/// Print a success message with a green checkmark
pub fn success(message: &str) {
    println!("{} {}", "✓".bright_green().bold(), message.bright_white());
}

/// Print a success message with additional details in dim text
pub fn success_with_details(message: &str, details: &str) {
    println!(
        "{} {} {}",
        "✓".bright_green().bold(),
        message.bright_white(),
        details.dimmed()
    );
}

/// Print an error message with a red X
pub fn error(message: &str) {
    eprintln!("{} {}", "✗".bright_red().bold(), message.bright_white());
}

/// Print a warning message with a yellow warning symbol
pub fn warning(message: &str) {
    println!("{} {}", "⚠".bright_yellow().bold(), message.bright_white());
}

/// Print an info message with a blue info symbol
pub fn info(message: &str) {
    println!("{} {}", "ℹ".bright_blue().bold(), message.bright_white());
}

/// Print a section header with a separator line
pub fn section(title: &str) {
    println!("\n{}", title.bright_cyan().bold());
    println!("{}", "─".repeat(50).dimmed());
}

/// Print a small section header without separator
pub fn subsection(title: &str) {
    println!("\n{}", title.bright_cyan());
}

/// Print a key-value pair with styled key and value
pub fn key_value(key: &str, value: &str) {
    println!(
        "  {} {}",
        format!("{}:", key).dimmed(),
        value.bright_white()
    );
}

/// Print a key-value pair where the value is highlighted
pub fn key_value_highlight(key: &str, value: &str) {
    println!(
        "  {} {}",
        format!("{}:", key).dimmed(),
        value.bright_cyan().bold()
    );
}

/// Print a label with colored value
pub fn label(text: &str, value: &str, color: LabelColor) {
    let styled_value = match color {
        LabelColor::Green => value.bright_green().bold().to_string(),
        LabelColor::Blue => value.bright_blue().bold().to_string(),
        LabelColor::Yellow => value.bright_yellow().bold().to_string(),
        LabelColor::Cyan => value.bright_cyan().bold().to_string(),
        LabelColor::Magenta => value.bright_magenta().bold().to_string(),
        LabelColor::White => value.bright_white().bold().to_string(),
    };
    println!("  {} {}", text.dimmed(), styled_value);
}

/// Colors for label values
pub enum LabelColor {
    Green,
    Blue,
    Yellow,
    Cyan,
    Magenta,
    White,
}

/// Print a step indicator
pub fn step(number: usize, total: usize, description: &str) {
    println!(
        "\n{} {}",
        format!("[{}/{}]", number, total).bright_magenta().bold(),
        description.bright_white()
    );
}

/// Print a dimmed/muted message
pub fn dimmed(message: &str) {
    println!("{}", message.dimmed());
}

/// Print a code/path element
pub fn code(text: &str) {
    println!("  {}", text.bright_yellow());
}

/// Print a path with proper styling
pub fn path(path_str: &str) {
    println!("  {}", path_str.bright_cyan());
}

/// Print a list item with a bullet
pub fn list_item(text: &str) {
    println!("  {} {}", "•".bright_white(), text.bright_white());
}

/// Print a list item with custom bullet and color
pub fn list_item_colored(text: &str, color: LabelColor) {
    let styled_text = match color {
        LabelColor::Green => text.bright_green().to_string(),
        LabelColor::Blue => text.bright_blue().to_string(),
        LabelColor::Yellow => text.bright_yellow().to_string(),
        LabelColor::Cyan => text.bright_cyan().to_string(),
        LabelColor::Magenta => text.bright_magenta().to_string(),
        LabelColor::White => text.bright_white().to_string(),
    };
    println!("  {} {}", "•".dimmed(), styled_text);
}

/// Print a summary box with a title and items
pub fn summary_box(title: &str, items: &[(String, String)]) {
    println!("\n{}", title.bright_cyan().bold());
    println!(
        "{}",
        "┌".dimmed().to_string() + &"─".repeat(48).dimmed().to_string() + &"┐".dimmed().to_string()
    );
    for (key, value) in items {
        let line = format!("│ {}: {}", key, value);
        let padding = 50_usize.saturating_sub(line.len() - 2); // Subtract ANSI codes approximation
        let key_value_str = format!("{}: {}", key.dimmed(), value.bright_white());
        let padding_str = format!("{:width$}│", "", width = padding)
            .dimmed()
            .to_string();
        println!("{} {} {}", "│".dimmed(), key_value_str, padding_str);
    }
    println!(
        "{}",
        "└".dimmed().to_string() + &"─".repeat(48).dimmed().to_string() + &"┘".dimmed().to_string()
    );
}

/// Print a horizontal separator
pub fn separator() {
    println!("{}", "─".repeat(50).dimmed());
}

/// Print a blank line for spacing
pub fn blank() {
    println!();
}

/// Print a command suggestion
pub fn command_suggestion(description: &str, command: &str) {
    println!(
        "  {} {}",
        description.dimmed(),
        command.bright_yellow().bold()
    );
}

/// Print next steps section
pub fn next_steps(steps: &[String]) {
    subsection("Next steps");
    for (i, step) in steps.iter().enumerate() {
        println!(
            "  {} {}",
            format!("{}.", i + 1).bright_magenta(),
            step.bright_white()
        );
    }
}

/// Print a progress indicator
pub fn progress(current: usize, total: usize, item_name: &str) {
    println!(
        "  {} {} {}",
        format!("[{}/{}]", current, total).dimmed(),
        "Processing".dimmed(),
        item_name.bright_white()
    );
}

/// Print a resource card (for project/template display)
pub fn resource_card(name: &str, kind: &str, description: Option<&str>) {
    println!(
        "\n{} {}",
        "▸".bright_blue().bold(),
        name.bright_cyan().bold()
    );
    println!("  {} {}", "Kind:".dimmed(), kind.bright_magenta());
    if let Some(desc) = description {
        println!("  {} {}", "Description:".dimmed(), desc.bright_white());
    }
}

/// Print a status check result
pub fn status_check(item: &str, available: bool) {
    if available {
        println!(
            "  {} {} {}",
            "✓".bright_green().bold(),
            item.bright_white(),
            "available".dimmed()
        );
    } else {
        println!(
            "  {} {} {}",
            "✗".bright_red().bold(),
            item.bright_white(),
            "not found".dimmed()
        );
    }
}

/// Print environment badge
pub fn environment_badge(env_name: &str) {
    println!(
        "  {} {}",
        "Environment:".dimmed(),
        env_name.bright_green().bold()
    );
}

/// Print a table header
pub fn table_header(columns: &[&str]) {
    let header = columns
        .iter()
        .map(|c| c.bright_cyan().bold().to_string())
        .collect::<Vec<_>>()
        .join(" │ ");
    println!("  {}", header);
    println!("  {}", "─".repeat(70).dimmed());
}

/// Print a table row
pub fn table_row(values: &[&str]) {
    let row = values
        .iter()
        .map(|v| v.bright_white().to_string())
        .collect::<Vec<_>>()
        .join(" │ ");
    println!("  {}", row);
}
