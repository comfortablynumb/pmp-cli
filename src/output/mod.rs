//! Beautiful terminal output styling for PMP CLI
//!
//! This module provides styled output functions inspired by Claude Code's aesthetic.

#![allow(dead_code)]

use owo_colors::OwoColorize;

/// Print a success message with a green checkmark
pub fn success(message: &str) {
    // Pastel mint green: RGB(152, 225, 152)
    println!(
        "{} {}",
        "✓".truecolor(152, 225, 152).bold(),
        message.bright_white()
    );
}

/// Print a success message with additional details in dim text
pub fn success_with_details(message: &str, details: &str) {
    // Pastel mint green: RGB(152, 225, 152)
    // Brighter grey: RGB(160, 160, 160)
    println!(
        "{} {} {}",
        "✓".truecolor(152, 225, 152).bold(),
        message.bright_white(),
        details.truecolor(160, 160, 160)
    );
}

/// Print an error message with a red X
pub fn error(message: &str) {
    // Pastel coral/salmon: RGB(255, 160, 160)
    eprintln!(
        "{} {}",
        "✗".truecolor(255, 160, 160).bold(),
        message.bright_white()
    );
}

/// Print a warning message with a yellow warning symbol
pub fn warning(message: &str) {
    // Pastel cream/yellow: RGB(255, 230, 160)
    println!(
        "{} {}",
        "⚠".truecolor(255, 230, 160).bold(),
        message.bright_white()
    );
}

/// Print an info message with a blue info symbol
pub fn info(message: &str) {
    // Pastel sky blue: RGB(160, 200, 255)
    println!(
        "{} {}",
        "ℹ".truecolor(160, 200, 255).bold(),
        message.bright_white()
    );
}

/// Print a section header with a separator line
pub fn section(title: &str) {
    // Pastel lavender: RGB(181, 174, 254)
    println!("\n{}", title.truecolor(181, 174, 254).bold());
    // Brighter grey: RGB(160, 160, 160)
    println!("{}", "─".repeat(50).truecolor(160, 160, 160));
}

/// Print a small section header without separator
pub fn subsection(title: &str) {
    // Softer pastel teal: RGB(120, 180, 195)
    println!("\n{}", title.truecolor(120, 180, 195));
    // Less intense separator - dots in brighter grey: RGB(160, 160, 160)
    println!("{}", "·".repeat(30).truecolor(160, 160, 160));
}

/// Print a key-value pair with styled key and value
pub fn key_value(key: &str, value: &str) {
    // Brighter grey: RGB(160, 160, 160)
    println!(
        "  {} {}",
        format!("{}:", key).truecolor(160, 160, 160),
        value.bright_white()
    );
}

/// Print a key-value pair where the value is highlighted
pub fn key_value_highlight(key: &str, value: &str) {
    // Softer pastel teal: RGB(120, 180, 195)
    println!(
        "  {} {}",
        format!("{}:", key).truecolor(160, 160, 160),
        value.truecolor(120, 180, 195).bold()
    );
}

/// Print a label with colored value
pub fn label(text: &str, value: &str, color: LabelColor) {
    let styled_value = match color {
        // Pastel mint green: RGB(152, 225, 152)
        LabelColor::Green => value.truecolor(152, 225, 152).bold().to_string(),
        // Pastel sky blue: RGB(160, 200, 255)
        LabelColor::Blue => value.truecolor(160, 200, 255).bold().to_string(),
        // Pastel cream/yellow: RGB(255, 230, 160)
        LabelColor::Yellow => value.truecolor(255, 230, 160).bold().to_string(),
        // Softer pastel teal: RGB(120, 180, 195)
        LabelColor::Cyan => value.truecolor(120, 180, 195).bold().to_string(),
        // Pastel lavender: RGB(181, 174, 254)
        LabelColor::Magenta => value.truecolor(181, 174, 254).bold().to_string(),
        LabelColor::White => value.bright_white().bold().to_string(),
    };
    // Brighter grey: RGB(160, 160, 160)
    println!("  {} {}", text.truecolor(160, 160, 160), styled_value);
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
    // Pastel lavender: RGB(181, 174, 254)
    println!(
        "\n{} {}",
        format!("[{}/{}]", number, total)
            .truecolor(181, 174, 254)
            .bold(),
        description.bright_white()
    );
}

/// Print a dimmed/muted message
pub fn dimmed(message: &str) {
    // Brighter grey: RGB(160, 160, 160)
    println!("{}", message.truecolor(160, 160, 160));
}

/// Print a message in dark yellow
pub fn dark_yellow(message: &str) {
    println!("{}", message.yellow());
}

/// Print a message in bright cyan (for highlighting URLs, paths, etc.)
pub fn cyan(message: &str) {
    println!("{}", message.bright_cyan());
}

/// Print a message in bright white (for titles and emphasis)
pub fn bright_white(message: &str) {
    println!("{}", message.bright_white());
}

/// Print a message in lavender/light purple (for values like URLs, paths, etc.)
pub fn lavender(message: &str) {
    // Pastel lavender: RGB(181, 174, 254) - soft, easy on the eyes
    println!("{}", message.truecolor(181, 174, 254));
}

/// Print a code/path element
pub fn code(text: &str) {
    // Pastel cream/yellow: RGB(255, 230, 160)
    println!("  {}", text.truecolor(255, 230, 160));
}

/// Print a path with proper styling
pub fn path(path_str: &str) {
    // Softer pastel teal: RGB(120, 180, 195)
    println!("  {}", path_str.truecolor(120, 180, 195));
}

/// Print a list item with a bullet
pub fn list_item(text: &str) {
    println!("  {} {}", "•".bright_white(), text.bright_white());
}

/// Print a list item with custom bullet and color
pub fn list_item_colored(text: &str, color: LabelColor) {
    let styled_text = match color {
        // Pastel mint green: RGB(152, 225, 152)
        LabelColor::Green => text.truecolor(152, 225, 152).to_string(),
        // Pastel sky blue: RGB(160, 200, 255)
        LabelColor::Blue => text.truecolor(160, 200, 255).to_string(),
        // Pastel cream/yellow: RGB(255, 230, 160)
        LabelColor::Yellow => text.truecolor(255, 230, 160).to_string(),
        // Softer pastel teal: RGB(120, 180, 195)
        LabelColor::Cyan => text.truecolor(120, 180, 195).to_string(),
        // Pastel lavender: RGB(181, 174, 254)
        LabelColor::Magenta => text.truecolor(181, 174, 254).to_string(),
        LabelColor::White => text.bright_white().to_string(),
    };
    // Brighter grey: RGB(160, 160, 160)
    println!("  {} {}", "•".truecolor(160, 160, 160), styled_text);
}

/// Print a summary box with a title and items
pub fn summary_box(title: &str, items: &[(String, String)]) {
    // Softer pastel teal: RGB(120, 180, 195)
    println!("\n{}", title.truecolor(120, 180, 195).bold());
    // Brighter grey: RGB(160, 160, 160)
    println!(
        "{}",
        "┌".truecolor(160, 160, 160).to_string()
            + &"─".repeat(48).truecolor(160, 160, 160).to_string()
            + &"┐".truecolor(160, 160, 160).to_string()
    );
    for (key, value) in items {
        let line = format!("│ {}: {}", key, value);
        let padding = 50_usize.saturating_sub(line.len() - 2); // Subtract ANSI codes approximation
        let key_value_str = format!("{}: {}", key.truecolor(160, 160, 160), value.bright_white());
        let padding_str = format!("{:width$}│", "", width = padding)
            .truecolor(160, 160, 160)
            .to_string();
        println!(
            "{} {} {}",
            "│".truecolor(160, 160, 160),
            key_value_str,
            padding_str
        );
    }
    println!(
        "{}",
        "└".truecolor(160, 160, 160).to_string()
            + &"─".repeat(48).truecolor(160, 160, 160).to_string()
            + &"┘".truecolor(160, 160, 160).to_string()
    );
}

/// Print a horizontal separator
pub fn separator() {
    // Brighter grey: RGB(160, 160, 160)
    println!("{}", "─".repeat(50).truecolor(160, 160, 160));
}

/// Print a blank line for spacing
pub fn blank() {
    println!();
}

/// Print a command suggestion
pub fn command_suggestion(description: &str, command: &str) {
    // Pastel cream/yellow: RGB(255, 230, 160)
    // Brighter grey: RGB(160, 160, 160)
    println!(
        "  {} {}",
        description.truecolor(160, 160, 160),
        command.truecolor(255, 230, 160).bold()
    );
}

/// Print next steps section
pub fn next_steps(steps: &[String]) {
    subsection("Next steps");
    for (i, step) in steps.iter().enumerate() {
        // Pastel lavender: RGB(181, 174, 254)
        println!(
            "  {} {}",
            format!("{}.", i + 1).truecolor(181, 174, 254),
            step.bright_white()
        );
    }
}

/// Print a progress indicator
pub fn progress(current: usize, total: usize, item_name: &str) {
    // Brighter grey: RGB(160, 160, 160)
    println!(
        "  {} {} {}",
        format!("[{}/{}]", current, total).truecolor(160, 160, 160),
        "Processing".truecolor(160, 160, 160),
        item_name.bright_white()
    );
}

/// Print a resource card (for project/template display)
pub fn resource_card(name: &str, kind: &str, description: Option<&str>) {
    // Pastel sky blue: RGB(160, 200, 255)
    // Softer pastel teal: RGB(120, 180, 195)
    println!(
        "\n{} {}",
        "▸".truecolor(160, 200, 255).bold(),
        name.truecolor(120, 180, 195).bold()
    );
    // Pastel lavender: RGB(181, 174, 254)
    // Brighter grey: RGB(160, 160, 160)
    println!(
        "  {} {}",
        "Kind:".truecolor(160, 160, 160),
        kind.truecolor(181, 174, 254)
    );
    if let Some(desc) = description {
        println!(
            "  {} {}",
            "Description:".truecolor(160, 160, 160),
            desc.bright_white()
        );
    }
}

/// Print a status check result
pub fn status_check(item: &str, available: bool) {
    if available {
        // Pastel mint green: RGB(152, 225, 152)
        // Brighter grey: RGB(160, 160, 160)
        println!(
            "  {} {} {}",
            "✓".truecolor(152, 225, 152).bold(),
            item.bright_white(),
            "available".truecolor(160, 160, 160)
        );
    } else {
        // Pastel coral/salmon: RGB(255, 160, 160)
        // Brighter grey: RGB(160, 160, 160)
        println!(
            "  {} {} {}",
            "✗".truecolor(255, 160, 160).bold(),
            item.bright_white(),
            "not found".truecolor(160, 160, 160)
        );
    }
}

/// Print environment badge
pub fn environment_badge(env_name: &str) {
    // Pastel mint green: RGB(152, 225, 152)
    println!(
        "  {} {}",
        "Environment:".dimmed(),
        env_name.truecolor(152, 225, 152).bold()
    );
}

/// Print a table header
pub fn table_header(columns: &[&str]) {
    // Softer pastel teal: RGB(120, 180, 195)
    let header = columns
        .iter()
        .map(|c| c.truecolor(120, 180, 195).bold().to_string())
        .collect::<Vec<_>>()
        .join(" │ ");
    println!("  {}", header);
    // Brighter grey: RGB(160, 160, 160)
    println!("  {}", "─".repeat(70).truecolor(160, 160, 160));
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
