//! Template linting module
//!
//! Validates template packs for common issues including:
//! - Missing required fields
//! - Unused inputs
//! - Invalid input type configurations
//! - Handlebars syntax errors
//! - Circular inheritance detection
//! - Best practices warnings

use anyhow::Result;
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::discovery::{TemplateDiscovery, TemplateInfo, TemplatePackInfo};
use super::metadata::{InputType, TemplateResource};

// ============================================================================
// Types
// ============================================================================

/// Severity level for lint issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    /// Critical error that must be fixed
    Error,
    /// Warning that should be addressed
    Warning,
    /// Informational suggestion
    Info,
}

impl std::fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintSeverity::Error => write!(f, "error"),
            LintSeverity::Warning => write!(f, "warning"),
            LintSeverity::Info => write!(f, "info"),
        }
    }
}

/// Category of lint issue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintCategory {
    /// Missing required fields
    RequiredField,
    /// Unused inputs
    UnusedInput,
    /// Invalid input configuration
    InvalidInputConfig,
    /// Handlebars syntax error
    HandlebarsError,
    /// Circular inheritance
    CircularInheritance,
    /// Best practices
    BestPractice,
}

impl std::fmt::Display for LintCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintCategory::RequiredField => write!(f, "required-field"),
            LintCategory::UnusedInput => write!(f, "unused-input"),
            LintCategory::InvalidInputConfig => write!(f, "invalid-input-config"),
            LintCategory::HandlebarsError => write!(f, "handlebars-error"),
            LintCategory::CircularInheritance => write!(f, "circular-inheritance"),
            LintCategory::BestPractice => write!(f, "best-practice"),
        }
    }
}

/// A single lint issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintIssue {
    /// Severity of the issue
    pub severity: LintSeverity,
    /// Category of the issue
    pub category: LintCategory,
    /// Human-readable message
    pub message: String,
    /// File path where the issue was found (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Line number (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// Suggested fix (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl LintIssue {
    fn error(category: LintCategory, message: impl Into<String>) -> Self {
        Self {
            severity: LintSeverity::Error,
            category,
            message: message.into(),
            file: None,
            line: None,
            suggestion: None,
        }
    }

    fn warning(category: LintCategory, message: impl Into<String>) -> Self {
        Self {
            severity: LintSeverity::Warning,
            category,
            message: message.into(),
            file: None,
            line: None,
            suggestion: None,
        }
    }

    fn info(category: LintCategory, message: impl Into<String>) -> Self {
        Self {
            severity: LintSeverity::Info,
            category,
            message: message.into(),
            file: None,
            line: None,
            suggestion: None,
        }
    }

    fn with_file(mut self, file: impl Into<PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }

    fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Result of linting a template pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintResult {
    /// Template pack name
    pub pack_name: String,
    /// Template pack path
    pub pack_path: PathBuf,
    /// Issues found
    pub issues: Vec<LintIssue>,
    /// Templates linted
    pub templates_linted: usize,
    /// Plugins linted
    pub plugins_linted: usize,
}

impl LintResult {
    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == LintSeverity::Error)
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == LintSeverity::Warning)
    }

    /// Count issues by severity
    pub fn count_by_severity(&self, severity: LintSeverity) -> usize {
        self.issues.iter().filter(|i| i.severity == severity).count()
    }
}

/// Options for linting
#[derive(Debug, Clone, Default)]
pub struct LintOptions {
    /// Skip unused input detection (can be slow for large templates)
    pub skip_unused_inputs: bool,
    /// Skip handlebars syntax validation
    pub skip_handlebars: bool,
    /// Include info-level suggestions
    pub include_info: bool,
}

// ============================================================================
// Linter
// ============================================================================

/// Template pack linter
pub struct TemplateLinter;

impl TemplateLinter {
    /// Lint a template pack
    pub fn lint_pack(
        fs: &dyn crate::traits::FileSystem,
        output: &dyn crate::traits::Output,
        pack: &TemplatePackInfo,
        all_packs: &[TemplatePackInfo],
        options: &LintOptions,
    ) -> Result<LintResult> {
        let mut issues = Vec::new();
        let mut templates_linted = 0;
        let mut plugins_linted = 0;

        // Discover templates in the pack
        let templates = TemplateDiscovery::discover_templates_in_pack(fs, output, &pack.path)?;

        // Lint each template
        for template in &templates {
            let template_issues =
                Self::lint_template(fs, template, pack, all_packs, &templates, options)?;
            issues.extend(template_issues);
            templates_linted += 1;
        }

        // Discover and lint plugins
        let plugins = TemplateDiscovery::discover_plugins_in_pack(
            fs,
            output,
            &pack.path,
            &pack.resource.metadata.name,
        )?;

        for plugin in &plugins {
            let plugin_issues = Self::lint_plugin(fs, plugin, options)?;
            issues.extend(plugin_issues);
            plugins_linted += 1;
        }

        // Filter out info-level issues if not requested
        if !options.include_info {
            issues.retain(|i| i.severity != LintSeverity::Info);
        }

        Ok(LintResult {
            pack_name: pack.resource.metadata.name.clone(),
            pack_path: pack.path.clone(),
            issues,
            templates_linted,
            plugins_linted,
        })
    }

    /// Lint a single template
    fn lint_template(
        fs: &dyn crate::traits::FileSystem,
        template: &TemplateInfo,
        pack: &TemplatePackInfo,
        all_packs: &[TemplatePackInfo],
        all_templates: &[TemplateInfo],
        options: &LintOptions,
    ) -> Result<Vec<LintIssue>> {
        let mut issues = Vec::new();
        let template_file = template.path.join(".pmp.template.yaml");

        // 1. Required fields validation
        issues.extend(Self::validate_required_fields(&template.resource, &template_file));

        // 2. Input configuration validation
        issues.extend(Self::validate_input_configs(&template.resource, &template_file));

        // 3. Unused inputs detection
        if !options.skip_unused_inputs {
            issues.extend(Self::detect_unused_inputs(fs, template)?);
        }

        // 4. Handlebars syntax validation
        if !options.skip_handlebars {
            issues.extend(Self::validate_handlebars_syntax(fs, template)?);
        }

        // 5. Circular inheritance detection
        issues.extend(Self::detect_circular_inheritance(
            template,
            pack,
            all_packs,
            all_templates,
        )?);

        // 6. Best practices
        issues.extend(Self::check_best_practices(&template.resource, &template_file));

        Ok(issues)
    }

    /// Lint a plugin
    fn lint_plugin(
        fs: &dyn crate::traits::FileSystem,
        plugin: &super::discovery::PluginInfo,
        options: &LintOptions,
    ) -> Result<Vec<LintIssue>> {
        let mut issues = Vec::new();
        let plugin_file = plugin.path.join(".pmp.plugin.yaml");

        // Required fields for plugin
        if plugin.resource.metadata.name.is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Plugin is missing required field: metadata.name",
                )
                .with_file(&plugin_file),
            );
        }

        if plugin.resource.spec.role.is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Plugin is missing required field: spec.role",
                )
                .with_file(&plugin_file),
            );
        }

        // Input configuration validation
        for input in &plugin.resource.spec.inputs {
            issues.extend(Self::validate_single_input(input, &plugin_file));
        }

        // Best practices for plugins
        if plugin.resource.metadata.description.is_none() {
            issues.push(
                LintIssue::info(
                    LintCategory::BestPractice,
                    format!(
                        "Plugin '{}' is missing a description",
                        plugin.resource.metadata.name
                    ),
                )
                .with_file(&plugin_file)
                .with_suggestion("Add a description to metadata.description"),
            );
        }

        // Handlebars validation for plugin src files
        if !options.skip_handlebars {
            let src_dir = plugin.path.join("src");

            if fs.exists(&src_dir) {
                issues.extend(Self::validate_handlebars_in_dir(fs, &src_dir)?);
            }
        }

        Ok(issues)
    }

    // ========================================================================
    // Validation Functions
    // ========================================================================

    /// Validate required fields in template
    fn validate_required_fields(resource: &TemplateResource, file: &Path) -> Vec<LintIssue> {
        let mut issues = Vec::new();

        // Check top-level required fields
        if resource.api_version.is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Template is missing required field: apiVersion",
                )
                .with_file(file),
            );
        } else if resource.api_version != "pmp.io/v1" {
            issues.push(
                LintIssue::warning(
                    LintCategory::RequiredField,
                    format!(
                        "Unexpected apiVersion '{}', expected 'pmp.io/v1'",
                        resource.api_version
                    ),
                )
                .with_file(file),
            );
        }

        if resource.kind.is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Template is missing required field: kind",
                )
                .with_file(file),
            );
        } else if resource.kind != "Template" {
            issues.push(
                LintIssue::warning(
                    LintCategory::RequiredField,
                    format!("Unexpected kind '{}', expected 'Template'", resource.kind),
                )
                .with_file(file),
            );
        }

        // Check metadata
        if resource.metadata.name.is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Template is missing required field: metadata.name",
                )
                .with_file(file),
            );
        }

        // Check spec required fields
        if resource.spec.api_version.is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Template is missing required field: spec.apiVersion",
                )
                .with_file(file),
            );
        }

        if resource.spec.kind.is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Template is missing required field: spec.kind",
                )
                .with_file(file),
            );
        }

        // Validate spec.kind is alphanumeric
        if !resource.spec.kind.is_empty()
            && !resource.spec.kind.chars().all(|c| c.is_ascii_alphanumeric())
        {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    format!(
                        "spec.kind '{}' must be alphanumeric only",
                        resource.spec.kind
                    ),
                )
                .with_file(file)
                .with_suggestion("Use only letters and numbers (e.g., 'VPC', 'WebApp')"),
            );
        }

        // Executor is required
        if resource.spec.executor.name().is_empty() {
            issues.push(
                LintIssue::error(
                    LintCategory::RequiredField,
                    "Template is missing required field: spec.executor",
                )
                .with_file(file),
            );
        }

        issues
    }

    /// Validate input configurations
    fn validate_input_configs(resource: &TemplateResource, file: &Path) -> Vec<LintIssue> {
        let mut issues = Vec::new();

        for input in &resource.spec.inputs {
            issues.extend(Self::validate_single_input(input, file));
        }

        // Check for duplicate input names
        let mut seen_names = HashSet::new();

        for input in &resource.spec.inputs {
            if !seen_names.insert(&input.name) {
                issues.push(
                    LintIssue::error(
                        LintCategory::InvalidInputConfig,
                        format!("Duplicate input name: '{}'", input.name),
                    )
                    .with_file(file),
                );
            }
        }

        issues
    }

    /// Validate a single input definition
    fn validate_single_input(
        input: &super::metadata::InputDefinition,
        file: &Path,
    ) -> Vec<LintIssue> {
        let mut issues = Vec::new();

        // Input name is required
        if input.name.is_empty() {
            issues.push(
                LintIssue::error(LintCategory::InvalidInputConfig, "Input is missing a name")
                    .with_file(file),
            );
            return issues;
        }

        // Validate input type specific configurations
        if let Some(input_type) = &input.input_type {
            match input_type {
                InputType::Number { min, max, .. } => {
                    if let (Some(min_val), Some(max_val)) = (min, max) {
                        if min_val > max_val {
                            issues.push(
                                LintIssue::error(
                                    LintCategory::InvalidInputConfig,
                                    format!(
                                        "Input '{}': min ({}) is greater than max ({})",
                                        input.name, min_val, max_val
                                    ),
                                )
                                .with_file(file),
                            );
                        }
                    }
                }
                InputType::Select { options } | InputType::MultiSelect { options, .. } => {
                    if options.is_empty() {
                        issues.push(
                            LintIssue::error(
                                LintCategory::InvalidInputConfig,
                                format!("Input '{}': select/multiselect must have options", input.name),
                            )
                            .with_file(file),
                        );
                    }

                    // Check for duplicate option values
                    let mut seen_values = HashSet::new();

                    for option in options {
                        if !seen_values.insert(&option.value) {
                            issues.push(
                                LintIssue::warning(
                                    LintCategory::InvalidInputConfig,
                                    format!(
                                        "Input '{}': duplicate option value '{}'",
                                        input.name, option.value
                                    ),
                                )
                                .with_file(file),
                            );
                        }
                    }
                }
                InputType::List { min, max, .. } => {
                    if let (Some(min_val), Some(max_val)) = (min, max) {
                        if min_val > max_val {
                            issues.push(
                                LintIssue::error(
                                    LintCategory::InvalidInputConfig,
                                    format!(
                                        "Input '{}': min ({}) is greater than max ({})",
                                        input.name, min_val, max_val
                                    ),
                                )
                                .with_file(file),
                            );
                        }
                    }
                }
                InputType::Duration {
                    min_seconds,
                    max_seconds,
                } => {
                    if let (Some(min_val), Some(max_val)) = (min_seconds, max_seconds) {
                        if min_val > max_val {
                            issues.push(
                                LintIssue::error(
                                    LintCategory::InvalidInputConfig,
                                    format!(
                                        "Input '{}': min_seconds ({}) is greater than max_seconds ({})",
                                        input.name, min_val, max_val
                                    ),
                                )
                                .with_file(file),
                            );
                        }
                    }
                }
                // Port is a simple type without configuration options
                InputType::Object { fields } => {
                    // Validate nested fields
                    for field in fields {
                        issues.extend(Self::validate_single_input(field, file));
                    }
                }
                InputType::RepeatableObject { fields, min, max, .. } => {
                    // Validate min/max
                    if let (Some(min_val), Some(max_val)) = (min, max) {
                        if min_val > max_val {
                            issues.push(
                                LintIssue::error(
                                    LintCategory::InvalidInputConfig,
                                    format!(
                                        "Input '{}': min ({}) is greater than max ({})",
                                        input.name, min_val, max_val
                                    ),
                                )
                                .with_file(file),
                            );
                        }
                    }

                    // Validate nested fields
                    for field in fields {
                        issues.extend(Self::validate_single_input(field, file));
                    }
                }
                InputType::KeyValue { min, max, .. } => {
                    if let (Some(min_val), Some(max_val)) = (min, max) {
                        if min_val > max_val {
                            issues.push(
                                LintIssue::error(
                                    LintCategory::InvalidInputConfig,
                                    format!(
                                        "Input '{}': min ({}) is greater than max ({})",
                                        input.name, min_val, max_val
                                    ),
                                )
                                .with_file(file),
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        issues
    }

    /// Detect unused inputs
    fn detect_unused_inputs(
        fs: &dyn crate::traits::FileSystem,
        template: &TemplateInfo,
    ) -> Result<Vec<LintIssue>> {
        let mut issues = Vec::new();
        let src_dir = template.path.join("src");

        // Collect all input names
        let input_names: HashSet<String> = template
            .resource
            .spec
            .inputs
            .iter()
            .map(|i| i.name.clone())
            .collect();

        if input_names.is_empty() || !fs.exists(&src_dir) {
            return Ok(issues);
        }

        // Read all template files and collect used variables
        let mut used_vars = HashSet::new();
        let entries = fs.walk_dir(&src_dir, 100)?;

        for path in entries {
            if fs.is_file(&path) {
                if let Ok(content) = fs.read_to_string(&path) {
                    // Find {{variable}} and {{#if variable}} patterns
                    Self::extract_handlebars_variables(&content, &mut used_vars);
                    // Find ${var:variable} patterns
                    Self::extract_interpolation_variables(&content, &mut used_vars);
                }
            }
        }

        // Check for unused inputs
        for input_name in &input_names {
            // Skip built-in variables
            if input_name.starts_with('_') {
                continue;
            }

            if !used_vars.contains(input_name) {
                issues.push(
                    LintIssue::warning(
                        LintCategory::UnusedInput,
                        format!("Input '{}' is defined but not used in templates", input_name),
                    )
                    .with_file(template.path.join(".pmp.template.yaml"))
                    .with_suggestion("Remove the input or use it in template files"),
                );
            }
        }

        Ok(issues)
    }

    /// Extract Handlebars variable references from content
    fn extract_handlebars_variables(content: &str, vars: &mut HashSet<String>) {
        // Match {{variable}}, {{#if variable}}, {{#each variable}}, {{#eq variable ...}}
        let patterns = [
            r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}",                    // {{var}}
            r"\{\{#if\s+([a-zA-Z_][a-zA-Z0-9_]*)",                        // {{#if var}}
            r"\{\{#unless\s+([a-zA-Z_][a-zA-Z0-9_]*)",                    // {{#unless var}}
            r"\{\{#each\s+([a-zA-Z_][a-zA-Z0-9_]*)",                      // {{#each var}}
            r"\{\{#eq\s+([a-zA-Z_][a-zA-Z0-9_]*)",                        // {{#eq var ...}}
            r"\{\{#contains\s+([a-zA-Z_][a-zA-Z0-9_]*)",                  // {{#contains var ...}}
            r"\{\{bool\s+([a-zA-Z_][a-zA-Z0-9_]*)",                       // {{bool var}}
            r"\{\{json\s+([a-zA-Z_][a-zA-Z0-9_]*)",                       // {{json var}}
            r"\{\{secret\s+([a-zA-Z_][a-zA-Z0-9_]*)",                     // {{secret var}}
            r"\{\{k8s_name\s+([a-zA-Z_][a-zA-Z0-9_]*)",                   // {{k8s_name var}}
        ];

        for pattern in patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                for cap in regex.captures_iter(content) {
                    if let Some(m) = cap.get(1) {
                        vars.insert(m.as_str().to_string());
                    }
                }
            }
        }
    }

    /// Extract ${var:...} interpolation variables from content
    fn extract_interpolation_variables(content: &str, vars: &mut HashSet<String>) {
        // Match ${var:variable_name} and ${var:variable_name:default}
        if let Ok(regex) = regex::Regex::new(r"\$\{var:([a-zA-Z_][a-zA-Z0-9_]*)") {
            for cap in regex.captures_iter(content) {
                if let Some(m) = cap.get(1) {
                    vars.insert(m.as_str().to_string());
                }
            }
        }
    }

    /// Validate Handlebars syntax in template files
    fn validate_handlebars_syntax(
        fs: &dyn crate::traits::FileSystem,
        template: &TemplateInfo,
    ) -> Result<Vec<LintIssue>> {
        let src_dir = template.path.join("src");

        if !fs.exists(&src_dir) {
            return Ok(Vec::new());
        }

        Self::validate_handlebars_in_dir(fs, &src_dir)
    }

    /// Validate Handlebars syntax in a directory
    fn validate_handlebars_in_dir(
        fs: &dyn crate::traits::FileSystem,
        dir: &Path,
    ) -> Result<Vec<LintIssue>> {
        let mut issues = Vec::new();
        let entries = fs.walk_dir(dir, 100)?;

        let handlebars = Handlebars::new();

        for path in entries {
            if fs.is_file(&path) && path.extension().map_or(false, |e| e == "hbs") {
                if let Ok(content) = fs.read_to_string(&path) {
                    // Try to compile the template
                    if let Err(e) = handlebars.render_template(&content, &serde_json::json!({})) {
                        // Filter out "missing variable" errors - those are expected
                        let error_str = e.to_string();

                        if !error_str.contains("not found")
                            && !error_str.contains("Variable")
                            && !error_str.contains("missing")
                        {
                            issues.push(
                                LintIssue::error(
                                    LintCategory::HandlebarsError,
                                    format!("Handlebars syntax error: {}", e),
                                )
                                .with_file(&path),
                            );
                        }
                    }

                    // Check for common issues
                    issues.extend(Self::check_handlebars_common_issues(&content, &path));
                }
            }
        }

        Ok(issues)
    }

    /// Check for common Handlebars issues
    fn check_handlebars_common_issues(content: &str, file: &Path) -> Vec<LintIssue> {
        let mut issues = Vec::new();

        // Check for unclosed blocks
        let open_blocks = [
            ("{{#if", "{{/if}}"),
            ("{{#unless", "{{/unless}}"),
            ("{{#each", "{{/each}}"),
            ("{{#eq", "{{/eq}}"),
            ("{{#contains", "{{/contains}}"),
            ("{{#with", "{{/with}}"),
        ];

        for (open, close) in open_blocks {
            let open_count = content.matches(open).count();
            let close_count = content.matches(close).count();

            if open_count != close_count {
                issues.push(
                    LintIssue::error(
                        LintCategory::HandlebarsError,
                        format!(
                            "Mismatched Handlebars blocks: {} '{}' vs {} '{}'",
                            open_count, open, close_count, close
                        ),
                    )
                    .with_file(file),
                );
            }
        }

        // Check for triple braces (unescaped) - might be intentional but worth noting
        if content.contains("{{{") || content.contains("}}}") {
            issues.push(
                LintIssue::info(
                    LintCategory::BestPractice,
                    "Template uses triple braces ({{{...}}}) for unescaped output",
                )
                .with_file(file)
                .with_suggestion("Ensure unescaped output is intentional and safe"),
            );
        }

        issues
    }

    /// Detect circular inheritance
    fn detect_circular_inheritance(
        template: &TemplateInfo,
        pack: &TemplatePackInfo,
        all_packs: &[TemplatePackInfo],
        all_templates: &[TemplateInfo],
    ) -> Result<Vec<LintIssue>> {
        let mut issues = Vec::new();

        // If template doesn't extend anything, no circular inheritance possible
        let Some(extends) = &template.resource.spec.extends else {
            return Ok(issues);
        };

        // Build inheritance chain
        let mut visited = HashSet::new();
        let mut chain = Vec::new();
        let template_id = format!(
            "{}/{}",
            pack.resource.metadata.name, template.resource.metadata.name
        );

        visited.insert(template_id.clone());
        chain.push(template_id);

        // Follow the chain
        let mut current_extends = Some(extends.clone());

        while let Some(ext) = current_extends {
            let ext_id = format!("{}/{}", ext.template_pack, ext.template);

            if visited.contains(&ext_id) {
                chain.push(ext_id);
                issues.push(
                    LintIssue::error(
                        LintCategory::CircularInheritance,
                        format!("Circular inheritance detected: {}", chain.join(" -> ")),
                    )
                    .with_file(template.path.join(".pmp.template.yaml")),
                );
                break;
            }

            visited.insert(ext_id.clone());
            chain.push(ext_id);

            // Find the extended template
            let base_pack = all_packs
                .iter()
                .find(|p| p.resource.metadata.name == ext.template_pack);

            if let Some(base_pack) = base_pack {
                let base_template = all_templates.iter().find(|t| {
                    t.resource.metadata.name == ext.template
                        && t.path.starts_with(&base_pack.path)
                });

                current_extends = base_template.and_then(|t| t.resource.spec.extends.clone());
            } else {
                // Base pack not found - will be caught by inheritance resolution
                break;
            }
        }

        Ok(issues)
    }

    /// Check best practices
    fn check_best_practices(resource: &TemplateResource, file: &Path) -> Vec<LintIssue> {
        let mut issues = Vec::new();

        // Check for missing description
        if resource.metadata.description.is_none() {
            issues.push(
                LintIssue::info(
                    LintCategory::BestPractice,
                    format!(
                        "Template '{}' is missing a description",
                        resource.metadata.name
                    ),
                )
                .with_file(file)
                .with_suggestion("Add a description to metadata.description"),
            );
        }

        // Check for inputs without descriptions
        for input in &resource.spec.inputs {
            if input.description.is_none() {
                issues.push(
                    LintIssue::info(
                        LintCategory::BestPractice,
                        format!("Input '{}' is missing a description", input.name),
                    )
                    .with_file(file)
                    .with_suggestion("Add a description field to the input"),
                );
            }
        }

        // Check for inputs without defaults (might be intentional)
        let inputs_without_defaults: Vec<_> = resource
            .spec
            .inputs
            .iter()
            .filter(|i| i.default.is_none() && !i.name.starts_with('_'))
            .collect();

        if inputs_without_defaults.len() > 5 {
            issues.push(
                LintIssue::info(
                    LintCategory::BestPractice,
                    format!(
                        "Template has {} inputs without default values",
                        inputs_without_defaults.len()
                    ),
                )
                .with_file(file)
                .with_suggestion("Consider adding sensible defaults to improve UX"),
            );
        }

        // Check for very long input lists
        if resource.spec.inputs.len() > 20 {
            issues.push(
                LintIssue::info(
                    LintCategory::BestPractice,
                    format!(
                        "Template has {} inputs - consider using object types to group related inputs",
                        resource.spec.inputs.len()
                    ),
                )
                .with_file(file),
            );
        }

        issues
    }
}

// ============================================================================
// Output Formatting
// ============================================================================

/// Format lint results for display
pub struct LintFormatter;

impl LintFormatter {
    /// Format results as text
    pub fn format_text(result: &LintResult) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "Linting template pack: {} ({})\n",
            result.pack_name,
            result.pack_path.display()
        ));
        output.push_str(&format!(
            "Templates: {}, Plugins: {}\n\n",
            result.templates_linted, result.plugins_linted
        ));

        if result.issues.is_empty() {
            output.push_str("No issues found.\n");
            return output;
        }

        // Group by severity
        let errors: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Error)
            .collect();
        let warnings: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Warning)
            .collect();
        let infos: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Info)
            .collect();

        for issue in &errors {
            output.push_str(&Self::format_issue(issue));
        }

        for issue in &warnings {
            output.push_str(&Self::format_issue(issue));
        }

        for issue in &infos {
            output.push_str(&Self::format_issue(issue));
        }

        output.push_str(&format!(
            "\nSummary: {} error(s), {} warning(s), {} info(s)\n",
            errors.len(),
            warnings.len(),
            infos.len()
        ));

        output
    }

    /// Format a single issue
    fn format_issue(issue: &LintIssue) -> String {
        let mut output = String::new();

        let prefix = match issue.severity {
            LintSeverity::Error => "ERROR",
            LintSeverity::Warning => "WARNING",
            LintSeverity::Info => "INFO",
        };

        output.push_str(&format!("[{}] {}\n", prefix, issue.message));

        if let Some(file) = &issue.file {
            output.push_str(&format!("  File: {}\n", file.display()));
        }

        if let Some(suggestion) = &issue.suggestion {
            output.push_str(&format!("  Suggestion: {}\n", suggestion));
        }

        output.push('\n');
        output
    }

    /// Format results as JSON
    pub fn format_json(result: &LintResult) -> Result<String> {
        Ok(serde_json::to_string_pretty(result)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_severity_display() {
        assert_eq!(LintSeverity::Error.to_string(), "error");
        assert_eq!(LintSeverity::Warning.to_string(), "warning");
        assert_eq!(LintSeverity::Info.to_string(), "info");
    }

    #[test]
    fn test_lint_category_display() {
        assert_eq!(LintCategory::RequiredField.to_string(), "required-field");
        assert_eq!(LintCategory::UnusedInput.to_string(), "unused-input");
    }

    #[test]
    fn test_lint_issue_builder() {
        let issue = LintIssue::error(LintCategory::RequiredField, "Missing field")
            .with_file("/path/to/file.yaml")
            .with_suggestion("Add the field");

        assert_eq!(issue.severity, LintSeverity::Error);
        assert_eq!(issue.category, LintCategory::RequiredField);
        assert_eq!(issue.message, "Missing field");
        assert_eq!(
            issue.file,
            Some(PathBuf::from("/path/to/file.yaml"))
        );
        assert_eq!(issue.suggestion, Some("Add the field".to_string()));
    }

    #[test]
    fn test_lint_result_has_errors() {
        let result = LintResult {
            pack_name: "test".to_string(),
            pack_path: PathBuf::from("/test"),
            issues: vec![LintIssue::error(LintCategory::RequiredField, "Test error")],
            templates_linted: 1,
            plugins_linted: 0,
        };

        assert!(result.has_errors());
        assert!(!result.has_warnings());
        assert_eq!(result.count_by_severity(LintSeverity::Error), 1);
    }

    #[test]
    fn test_extract_handlebars_variables() {
        let content = r#"
            {{name}}
            {{#if enabled}}
            {{#each items}}
            {{bool active}}
            {{json config}}
        "#;

        let mut vars = HashSet::new();
        TemplateLinter::extract_handlebars_variables(content, &mut vars);

        assert!(vars.contains("name"));
        assert!(vars.contains("enabled"));
        assert!(vars.contains("items"));
        assert!(vars.contains("active"));
        assert!(vars.contains("config"));
    }

    #[test]
    fn test_extract_interpolation_variables() {
        let content = r#"
            ${var:project_name}
            ${var:region:us-east-1}
            ${env:AWS_REGION}
        "#;

        let mut vars = HashSet::new();
        TemplateLinter::extract_interpolation_variables(content, &mut vars);

        assert!(vars.contains("project_name"));
        assert!(vars.contains("region"));
        assert!(!vars.contains("AWS_REGION")); // env vars are not input vars
    }

    #[test]
    fn test_check_handlebars_common_issues_balanced() {
        let content = "{{#if foo}}bar{{/if}}";
        let issues = TemplateLinter::check_handlebars_common_issues(content, Path::new("test.hbs"));
        assert!(issues.is_empty());
    }

    #[test]
    fn test_check_handlebars_common_issues_unbalanced() {
        let content = "{{#if foo}}bar";
        let issues = TemplateLinter::check_handlebars_common_issues(content, Path::new("test.hbs"));
        assert!(issues.iter().any(|i| i.severity == LintSeverity::Error));
    }
}
