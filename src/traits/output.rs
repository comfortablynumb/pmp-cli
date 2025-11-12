use std::sync::Mutex;

/// Output message captured by MockOutput for testing
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum OutputMessage {
    Success(String),
    Error(String),
    Warning(String),
    Info(String),
    Section(String),
    Subsection(String),
    KeyValue(String, String),
    Dimmed(String),
    DarkYellow(String),
    Cyan(String),
    BrightWhite(String),
    Lavender(String),
    Blank,
}

/// Trait for terminal output operations to enable testing with mocks
pub trait Output: Send + Sync {
    /// Print a success message
    fn success(&self, message: &str);

    /// Print an error message
    #[allow(dead_code)]
    fn error(&self, message: &str);

    /// Print a warning message
    fn warning(&self, message: &str);

    /// Print an info message
    fn info(&self, message: &str);

    /// Print a section header
    fn section(&self, title: &str);

    /// Print a subsection header
    fn subsection(&self, title: &str);

    /// Print a key-value pair
    fn key_value(&self, key: &str, value: &str);

    /// Print a key-value pair with highlighted value
    fn key_value_highlight(&self, key: &str, value: &str);

    /// Print a dimmed/muted message
    fn dimmed(&self, message: &str);

    /// Print a message in dark yellow
    #[allow(dead_code)]
    fn dark_yellow(&self, message: &str);

    /// Print a message in bright cyan (for highlighting URLs, paths, etc.)
    #[allow(dead_code)]
    fn cyan(&self, message: &str);

    /// Print a message in bright white (for titles and emphasis)
    #[allow(dead_code)]
    fn bright_white(&self, message: &str);

    /// Print a message in lavender/light purple (for values like URLs, paths, etc.)
    fn lavender(&self, message: &str);

    /// Print a blank line
    fn blank(&self);

    /// Print an environment badge
    fn environment_badge(&self, env_name: &str);

    /// Print a status check result (e.g., for prerequisite checks)
    fn status_check(&self, item: &str, available: bool);
}

/// Real terminal output implementation using the output module
pub struct TerminalOutput;

impl Output for TerminalOutput {
    fn success(&self, message: &str) {
        crate::output::success(message);
    }

    fn error(&self, message: &str) {
        crate::output::error(message);
    }

    fn warning(&self, message: &str) {
        crate::output::warning(message);
    }

    fn info(&self, message: &str) {
        crate::output::info(message);
    }

    fn section(&self, title: &str) {
        crate::output::section(title);
    }

    fn subsection(&self, title: &str) {
        crate::output::subsection(title);
    }

    fn key_value(&self, key: &str, value: &str) {
        crate::output::key_value(key, value);
    }

    fn key_value_highlight(&self, key: &str, value: &str) {
        crate::output::key_value_highlight(key, value);
    }

    fn dimmed(&self, message: &str) {
        crate::output::dimmed(message);
    }

    fn dark_yellow(&self, message: &str) {
        crate::output::dark_yellow(message);
    }

    fn cyan(&self, message: &str) {
        crate::output::cyan(message);
    }

    fn bright_white(&self, message: &str) {
        crate::output::bright_white(message);
    }

    fn lavender(&self, message: &str) {
        crate::output::lavender(message);
    }

    fn blank(&self) {
        crate::output::blank();
    }

    fn environment_badge(&self, env_name: &str) {
        crate::output::environment_badge(env_name);
    }

    fn status_check(&self, item: &str, available: bool) {
        crate::output::status_check(item, available);
    }
}

/// Mock output implementation for testing (captures output)
#[allow(dead_code)]
pub struct MockOutput {
    messages: Mutex<Vec<OutputMessage>>,
}

#[allow(dead_code)]
impl MockOutput {
    /// Create new mock output
    pub fn new() -> Self {
        Self {
            messages: Mutex::new(Vec::new()),
        }
    }

    /// Get all captured messages
    pub fn get_messages(&self) -> Vec<OutputMessage> {
        self.messages.lock().unwrap().clone()
    }

    /// Check if a specific message was output
    pub fn contains_message(&self, message: &OutputMessage) -> bool {
        self.messages.lock().unwrap().contains(message)
    }

    /// Check if any success message was output
    pub fn has_success(&self) -> bool {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .any(|m| matches!(m, OutputMessage::Success(_)))
    }

    /// Check if any error message was output
    pub fn has_error(&self) -> bool {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .any(|m| matches!(m, OutputMessage::Error(_)))
    }

    /// Get all success messages
    pub fn get_successes(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter_map(|m| {
                if let OutputMessage::Success(msg) = m {
                    Some(msg.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all error messages
    pub fn get_errors(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter_map(|m| {
                if let OutputMessage::Error(msg) = m {
                    Some(msg.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clear all captured messages
    pub fn clear(&self) {
        self.messages.lock().unwrap().clear();
    }

    /// Get all messages formatted as text
    pub fn to_text(&self) -> String {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .map(|msg| match msg {
                OutputMessage::Success(s) => format!("✓ {}", s),
                OutputMessage::Error(s) => format!("✗ {}", s),
                OutputMessage::Warning(s) => format!("⚠ {}", s),
                OutputMessage::Info(s) => s.clone(),
                OutputMessage::Section(s) => format!("\n=== {} ===", s),
                OutputMessage::Subsection(s) => format!("\n--- {} ---", s),
                OutputMessage::KeyValue(k, v) => format!("{}: {}", k, v),
                OutputMessage::Dimmed(s) => s.clone(),
                OutputMessage::DarkYellow(s) => s.clone(),
                OutputMessage::Cyan(s) => s.clone(),
                OutputMessage::BrightWhite(s) => s.clone(),
                OutputMessage::Lavender(s) => s.clone(),
                OutputMessage::Blank => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for MockOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl Output for MockOutput {
    fn success(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Success(message.to_string()));
    }

    fn error(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Error(message.to_string()));
    }

    fn warning(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Warning(message.to_string()));
    }

    fn info(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Info(message.to_string()));
    }

    fn section(&self, title: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Section(title.to_string()));
    }

    fn subsection(&self, title: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Subsection(title.to_string()));
    }

    fn key_value(&self, key: &str, value: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::KeyValue(key.to_string(), value.to_string()));
    }

    fn key_value_highlight(&self, key: &str, value: &str) {
        // For testing purposes, treat the same as key_value
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::KeyValue(key.to_string(), value.to_string()));
    }

    fn dimmed(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Dimmed(message.to_string()));
    }

    fn dark_yellow(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::DarkYellow(message.to_string()));
    }

    fn cyan(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Cyan(message.to_string()));
    }

    fn bright_white(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::BrightWhite(message.to_string()));
    }

    fn lavender(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Lavender(message.to_string()));
    }

    fn blank(&self) {
        self.messages.lock().unwrap().push(OutputMessage::Blank);
    }

    fn environment_badge(&self, env_name: &str) {
        // Treat as a key-value for testing
        self.messages.lock().unwrap().push(OutputMessage::KeyValue(
            "Environment".to_string(),
            env_name.to_string(),
        ));
    }

    fn status_check(&self, item: &str, available: bool) {
        // Treat as an info message for testing
        let message = if available {
            format!("{} is available", item)
        } else {
            format!("{} is NOT available", item)
        };
        self.messages
            .lock()
            .unwrap()
            .push(OutputMessage::Info(message));
    }
}
