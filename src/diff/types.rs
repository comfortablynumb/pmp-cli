//! Data types for plan diff visualization
//!
//! This module defines the data structures used to represent parsed plan output
//! and render it in various formats.

use serde::{Deserialize, Serialize};

/// Represents the type of change for a resource
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffChangeType {
    /// Resource will be created
    Create,
    /// Resource will be updated in-place
    Update,
    /// Resource will be destroyed and recreated
    Replace,
    /// Resource will be destroyed
    Destroy,
    /// Data source read
    Read,
    /// No changes
    NoOp,
}

impl DiffChangeType {
    /// Get the symbol used to represent this change type
    pub fn symbol(&self) -> &'static str {
        match self {
            DiffChangeType::Create => "+",
            DiffChangeType::Update => "~",
            DiffChangeType::Replace => "±",
            DiffChangeType::Destroy => "-",
            DiffChangeType::Read => "≤",
            DiffChangeType::NoOp => " ",
        }
    }

    /// Get the label for this change type
    pub fn label(&self) -> &'static str {
        match self {
            DiffChangeType::Create => "will be created",
            DiffChangeType::Update => "will be updated",
            DiffChangeType::Replace => "must be replaced",
            DiffChangeType::Destroy => "will be destroyed",
            DiffChangeType::Read => "will be read",
            DiffChangeType::NoOp => "no changes",
        }
    }

    /// Get RGB color tuple for this change type
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            DiffChangeType::Create => (152, 225, 152),  // Pastel mint green
            DiffChangeType::Update => (255, 230, 160),  // Pastel cream/yellow
            DiffChangeType::Replace => (181, 174, 254), // Pastel lavender
            DiffChangeType::Destroy => (255, 160, 160), // Pastel coral
            DiffChangeType::Read => (160, 200, 255),    // Pastel sky blue
            DiffChangeType::NoOp => (160, 160, 160),    // Grey
        }
    }
}

/// Type of attribute change within a resource
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeChangeType {
    /// Attribute will be added
    Added,
    /// Attribute will be removed
    Removed,
    /// Attribute value will be modified
    Modified,
    /// Attribute is unchanged (for context)
    Unchanged,
}

impl AttributeChangeType {
    /// Get the symbol for this attribute change type
    pub fn symbol(&self) -> &'static str {
        match self {
            AttributeChangeType::Added => "+",
            AttributeChangeType::Removed => "-",
            AttributeChangeType::Modified => "~",
            AttributeChangeType::Unchanged => " ",
        }
    }

    /// Get RGB color tuple for this attribute change type
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            AttributeChangeType::Added => (152, 225, 152),   // Pastel mint green
            AttributeChangeType::Removed => (255, 160, 160), // Pastel coral
            AttributeChangeType::Modified => (255, 230, 160), // Pastel cream/yellow
            AttributeChangeType::Unchanged => (160, 160, 160), // Grey
        }
    }
}

/// A single attribute change within a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeChange {
    /// Name of the attribute (e.g., "ami", "instance_type")
    pub name: String,

    /// Type of change for this attribute
    pub change_type: AttributeChangeType,

    /// Old value (None for additions)
    pub old_value: Option<String>,

    /// New value (None for removals)
    pub new_value: Option<String>,

    /// Whether this attribute is marked as sensitive
    pub sensitive: bool,

    /// Whether value is known only after apply
    pub computed: bool,

    /// Whether this attribute forces resource replacement
    pub forces_replacement: bool,
}

impl AttributeChange {
    /// Create a new attribute change
    pub fn new(name: &str, change_type: AttributeChangeType) -> Self {
        Self {
            name: name.to_string(),
            change_type,
            old_value: None,
            new_value: None,
            sensitive: false,
            computed: false,
            forces_replacement: false,
        }
    }

    /// Set the old value
    pub fn with_old_value(mut self, value: &str) -> Self {
        self.old_value = Some(value.to_string());
        self
    }

    /// Set the new value
    pub fn with_new_value(mut self, value: &str) -> Self {
        self.new_value = Some(value.to_string());
        self
    }

    /// Mark as sensitive
    pub fn with_sensitive(mut self, sensitive: bool) -> Self {
        self.sensitive = sensitive;
        self
    }

    /// Mark as computed
    pub fn with_computed(mut self, computed: bool) -> Self {
        self.computed = computed;
        self
    }

    /// Mark as forcing replacement
    pub fn with_forces_replacement(mut self, forces: bool) -> Self {
        self.forces_replacement = forces;
        self
    }
}

/// A resource change block in the plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChange {
    /// Full resource address (e.g., "aws_instance.example", "module.vpc.aws_subnet.main")
    pub address: String,

    /// Resource type (e.g., "aws_instance")
    pub resource_type: String,

    /// Resource name (e.g., "example")
    pub resource_name: String,

    /// Module path if applicable (e.g., "module.vpc")
    pub module_path: Option<String>,

    /// Type of change for this resource
    pub change_type: DiffChangeType,

    /// Attribute changes within this resource
    pub attributes: Vec<AttributeChange>,

    /// Attributes that force replacement (if any)
    pub forces_replacement: Vec<String>,
}

impl ResourceChange {
    /// Create a new resource change
    pub fn new(address: &str, change_type: DiffChangeType) -> Self {
        let (module_path, resource_type, resource_name) = Self::parse_address(address);

        Self {
            address: address.to_string(),
            resource_type,
            resource_name,
            module_path,
            change_type,
            attributes: Vec::new(),
            forces_replacement: Vec::new(),
        }
    }

    /// Parse a resource address into its components
    fn parse_address(address: &str) -> (Option<String>, String, String) {
        // Handle module prefixes: module.vpc.aws_instance.example
        let parts: Vec<&str> = address.split('.').collect();

        if parts.len() >= 4 && parts[0] == "module" {
            // Has module prefix
            let module_parts: Vec<&str> = parts.iter().take(parts.len() - 2).cloned().collect();
            let module_path = module_parts.join(".");
            let resource_type = parts[parts.len() - 2].to_string();
            let resource_name = parts[parts.len() - 1].to_string();
            (Some(module_path), resource_type, resource_name)
        } else if parts.len() >= 2 {
            // No module prefix: aws_instance.example
            let resource_type = parts[0].to_string();
            let resource_name = parts[1..].join(".");
            (None, resource_type, resource_name)
        } else {
            // Fallback
            (None, address.to_string(), String::new())
        }
    }

    /// Add an attribute change
    pub fn add_attribute(&mut self, attr: AttributeChange) {
        if attr.forces_replacement {
            self.forces_replacement.push(attr.name.clone());
        }
        self.attributes.push(attr);
    }
}

/// Summary statistics for the plan
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanSummary {
    /// Number of resources to add
    pub to_add: usize,

    /// Number of resources to change
    pub to_change: usize,

    /// Number of resources to destroy
    pub to_destroy: usize,

    /// Number of resources to replace
    pub to_replace: usize,

    /// Number of unchanged resources
    pub unchanged: usize,
}

impl PlanSummary {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        self.to_add > 0 || self.to_change > 0 || self.to_destroy > 0 || self.to_replace > 0
    }

    /// Get total number of changes
    pub fn total_changes(&self) -> usize {
        self.to_add + self.to_change + self.to_destroy + self.to_replace
    }
}

/// Parsed plan output ready for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPlan {
    /// Resource changes in the plan
    pub resources: Vec<ResourceChange>,

    /// Summary statistics
    pub summary: PlanSummary,

    /// Whether there are any changes
    pub has_changes: bool,

    /// Raw output for fallback display
    #[serde(skip)]
    pub raw_output: String,
}

impl ParsedPlan {
    /// Create a new parsed plan
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            summary: PlanSummary::default(),
            has_changes: false,
            raw_output: String::new(),
        }
    }

    /// Add a resource change and update summary
    pub fn add_resource(&mut self, resource: ResourceChange) {
        match resource.change_type {
            DiffChangeType::Create => self.summary.to_add += 1,
            DiffChangeType::Update => self.summary.to_change += 1,
            DiffChangeType::Destroy => self.summary.to_destroy += 1,
            DiffChangeType::Replace => self.summary.to_replace += 1,
            DiffChangeType::NoOp => self.summary.unchanged += 1,
            DiffChangeType::Read => {}
        }
        self.has_changes = self.summary.has_changes();
        self.resources.push(resource);
    }
}

impl Default for ParsedPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// Options for diff rendering
#[derive(Debug, Clone)]
pub struct DiffRenderOptions {
    /// Show unchanged attributes (for context)
    pub show_unchanged: bool,

    /// Use compact output (no extra spacing)
    pub compact_mode: bool,

    /// Use side-by-side view (ASCII only)
    pub side_by_side: bool,

    /// Maximum width for values before truncation
    pub max_value_width: usize,

    /// Show sensitive values (normally hidden)
    pub show_sensitive: bool,

    /// Terminal width for formatting
    pub terminal_width: usize,
}

impl Default for DiffRenderOptions {
    fn default() -> Self {
        Self {
            show_unchanged: false,
            compact_mode: false,
            side_by_side: false,
            max_value_width: 60,
            show_sensitive: false,
            terminal_width: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_change_type_symbol() {
        assert_eq!(DiffChangeType::Create.symbol(), "+");
        assert_eq!(DiffChangeType::Update.symbol(), "~");
        assert_eq!(DiffChangeType::Destroy.symbol(), "-");
        assert_eq!(DiffChangeType::Replace.symbol(), "±");
    }

    #[test]
    fn test_attribute_change_builder() {
        let attr = AttributeChange::new("ami", AttributeChangeType::Modified)
            .with_old_value("ami-old")
            .with_new_value("ami-new")
            .with_forces_replacement(true);

        assert_eq!(attr.name, "ami");
        assert_eq!(attr.old_value, Some("ami-old".to_string()));
        assert_eq!(attr.new_value, Some("ami-new".to_string()));
        assert!(attr.forces_replacement);
    }

    #[test]
    fn test_resource_address_parsing() {
        let resource = ResourceChange::new("aws_instance.example", DiffChangeType::Create);
        assert_eq!(resource.resource_type, "aws_instance");
        assert_eq!(resource.resource_name, "example");
        assert!(resource.module_path.is_none());

        let module_resource =
            ResourceChange::new("module.vpc.aws_subnet.main", DiffChangeType::Update);
        assert_eq!(module_resource.module_path, Some("module.vpc".to_string()));
        assert_eq!(module_resource.resource_type, "aws_subnet");
        assert_eq!(module_resource.resource_name, "main");
    }

    #[test]
    fn test_plan_summary() {
        let mut plan = ParsedPlan::new();
        assert!(!plan.has_changes);

        plan.add_resource(ResourceChange::new("aws_instance.a", DiffChangeType::Create));
        plan.add_resource(ResourceChange::new("aws_instance.b", DiffChangeType::Update));
        plan.add_resource(ResourceChange::new("aws_instance.c", DiffChangeType::Destroy));

        assert!(plan.has_changes);
        assert_eq!(plan.summary.to_add, 1);
        assert_eq!(plan.summary.to_change, 1);
        assert_eq!(plan.summary.to_destroy, 1);
        assert_eq!(plan.summary.total_changes(), 3);
    }
}
