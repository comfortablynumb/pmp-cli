use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Mutex;

/// Response type for mock user input
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum MockResponse {
    Select(String),
    MultiSelect(Vec<String>),
    Text(String),
    Password(String),
    Confirm(bool),
}

/// Trait for user input operations to enable testing with mocks
pub trait UserInput: Send + Sync {
    /// Display a selection prompt with options
    fn select(&self, prompt: &str, options: Vec<String>, default: Option<usize>) -> Result<String>;

    /// Display a multi-selection prompt with options
    fn multi_select(
        &self,
        prompt: &str,
        options: Vec<String>,
        defaults: Option<&[usize]>,
    ) -> Result<Vec<String>>;

    /// Display a text input prompt
    fn text(&self, prompt: &str, default: Option<&str>) -> Result<String>;

    /// Display a password input prompt (hidden text)
    fn password(&self, prompt: &str) -> Result<String>;

    /// Display a confirmation prompt (yes/no)
    /// If default is Some(value), allow Enter to accept the default
    /// If default is None, require explicit Y/N input
    fn confirm(&self, prompt: &str, default: Option<bool>) -> Result<bool>;
}

/// Real user input implementation using inquire crate
pub struct InquireUserInput;

impl UserInput for InquireUserInput {
    fn select(&self, prompt: &str, options: Vec<String>, default: Option<usize>) -> Result<String> {
        use inquire::Select;
        let mut prompt_builder = Select::new(prompt, options);
        if let Some(default_index) = default {
            prompt_builder = prompt_builder.with_starting_cursor(default_index);
        }
        let answer = prompt_builder.prompt()?;
        Ok(answer)
    }

    fn multi_select(
        &self,
        prompt: &str,
        options: Vec<String>,
        defaults: Option<&[usize]>,
    ) -> Result<Vec<String>> {
        use inquire::MultiSelect;
        let mut prompt_builder = MultiSelect::new(prompt, options);
        if let Some(default_indices) = defaults {
            prompt_builder = prompt_builder.with_default(default_indices);
        }
        let answer = prompt_builder.prompt()?;
        Ok(answer)
    }

    fn text(&self, prompt: &str, default: Option<&str>) -> Result<String> {
        use inquire::Text;
        let mut text_prompt = Text::new(prompt);
        if let Some(default_val) = default {
            text_prompt = text_prompt.with_default(default_val);
        }
        let answer = text_prompt.prompt()?;
        Ok(answer)
    }

    fn password(&self, prompt: &str) -> Result<String> {
        use inquire::Password;
        let answer = Password::new(prompt).without_confirmation().prompt()?;
        Ok(answer)
    }

    fn confirm(&self, prompt: &str, default: Option<bool>) -> Result<bool> {
        use inquire::Confirm;

        let answer = match default {
            Some(value) => {
                // Default provided - allow Enter to accept it
                Confirm::new(prompt).with_default(value).prompt()?
            }
            None => {
                // No default - require explicit Y/N input
                let help_msg = "Please enter Y or N explicitly";
                Confirm::new(prompt)
                    .with_help_message(help_msg)
                    .prompt()?
            }
        };

        Ok(answer)
    }
}

/// Mock user input implementation for testing
#[allow(dead_code)]
pub struct MockUserInput {
    responses: Mutex<VecDeque<MockResponse>>,
}

#[allow(dead_code)]
impl MockUserInput {
    /// Create new mock with no pre-configured responses
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
        }
    }

    /// Create mock with pre-configured responses
    pub fn with_responses(responses: Vec<MockResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }

    /// Add a response to the queue
    pub fn add_response(&self, response: MockResponse) {
        self.responses.lock().unwrap().push_back(response);
    }

    /// Get the next response from the queue
    fn next_response(&self) -> Result<MockResponse> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("No more mock responses available"))
    }
}

impl Default for MockUserInput {
    fn default() -> Self {
        Self::new()
    }
}

impl UserInput for MockUserInput {
    fn select(&self, _prompt: &str, options: Vec<String>, _default: Option<usize>) -> Result<String> {
        match self.next_response()? {
            MockResponse::Select(answer) => {
                // Verify the answer is in the options
                if options.contains(&answer) {
                    Ok(answer)
                } else {
                    anyhow::bail!(
                        "Mock response '{}' is not in the provided options: {:?}",
                        answer,
                        options
                    )
                }
            }
            _ => anyhow::bail!("Expected Select response but got a different type"),
        }
    }

    fn multi_select(
        &self,
        _prompt: &str,
        options: Vec<String>,
        _defaults: Option<&[usize]>,
    ) -> Result<Vec<String>> {
        match self.next_response()? {
            MockResponse::MultiSelect(answers) => {
                // Verify all answers are in the options
                for answer in &answers {
                    if !options.contains(answer) {
                        anyhow::bail!(
                            "Mock response '{}' is not in the provided options: {:?}",
                            answer,
                            options
                        )
                    }
                }
                Ok(answers)
            }
            _ => anyhow::bail!("Expected MultiSelect response but got a different type"),
        }
    }

    fn text(&self, _prompt: &str, _default: Option<&str>) -> Result<String> {
        match self.next_response()? {
            MockResponse::Text(answer) => Ok(answer),
            _ => anyhow::bail!("Expected Text response but got a different type"),
        }
    }

    fn password(&self, _prompt: &str) -> Result<String> {
        match self.next_response()? {
            MockResponse::Password(answer) => Ok(answer),
            _ => anyhow::bail!("Expected Password response but got a different type"),
        }
    }

    fn confirm(&self, _prompt: &str, default: Option<bool>) -> Result<bool> {
        match default {
            Some(value) => Ok(value), // Auto-use default without consuming mock response
            None => match self.next_response()? {
                MockResponse::Confirm(answer) => Ok(answer),
                _ => anyhow::bail!("Expected Confirm response but got a different type"),
            },
        }
    }
}
