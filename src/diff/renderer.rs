//! Renderers for plan diff visualization
//!
//! This module provides ASCII (terminal) and HTML renderers for
//! displaying plan output in a formatted, color-coded manner.

use super::types::{
    AttributeChange, AttributeChangeType, DiffChangeType, DiffRenderOptions, ParsedPlan,
    PlanSummary, ResourceChange,
};

/// Trait for diff renderers
pub trait DiffRenderer {
    /// Render the parsed plan to a string
    fn render(&self, plan: &ParsedPlan, options: &DiffRenderOptions) -> String;
}

/// ASCII renderer for terminal output
pub struct AsciiRenderer;

impl Default for AsciiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl AsciiRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render summary section
    fn render_summary(&self, summary: &PlanSummary) -> String {
        let mut output = String::new();

        output.push_str("Plan Summary:\n");

        let mut parts = Vec::new();

        if summary.to_add > 0 {
            parts.push(format!("+{} to add", summary.to_add));
        }

        if summary.to_change > 0 {
            parts.push(format!("~{} to change", summary.to_change));
        }

        if summary.to_replace > 0 {
            parts.push(format!("±{} to replace", summary.to_replace));
        }

        if summary.to_destroy > 0 {
            parts.push(format!("-{} to destroy", summary.to_destroy));
        }

        if parts.is_empty() {
            output.push_str("  No changes.\n");
        } else {
            output.push_str(&format!("  {}\n", parts.join(", ")));
        }

        output.push('\n');
        output
    }

    /// Render a single resource change
    fn render_resource(&self, resource: &ResourceChange, options: &DiffRenderOptions) -> String {
        let mut output = String::new();

        let symbol = resource.change_type.symbol();
        let label = resource.change_type.label();

        output.push_str(&format!("{} {} ({})\n", symbol, resource.address, label));

        // Render attributes
        for attr in &resource.attributes {
            if attr.change_type == AttributeChangeType::Unchanged && !options.show_unchanged {
                continue;
            }

            output.push_str(&self.render_attribute(attr, options));
        }

        if !options.compact_mode {
            output.push('\n');
        }

        output
    }

    /// Render a single attribute change
    fn render_attribute(&self, attr: &AttributeChange, options: &DiffRenderOptions) -> String {
        let symbol = attr.change_type.symbol();
        let mut line = format!("    {} {}", symbol, attr.name);

        // Add value information
        match attr.change_type {
            AttributeChangeType::Added => {
                if let Some(ref value) = attr.new_value {
                    let display_value = self.format_value(value, attr, options);
                    line.push_str(&format!(" = {}", display_value));
                }
            }
            AttributeChangeType::Removed => {
                if let Some(ref value) = attr.old_value {
                    let display_value = self.format_value(value, attr, options);
                    line.push_str(&format!(" = {}", display_value));
                }
            }
            AttributeChangeType::Modified => {
                let old = attr.old_value.as_deref().unwrap_or("(unknown)");
                let new = attr.new_value.as_deref().unwrap_or("(unknown)");
                let old_display = self.format_value(old, attr, options);
                let new_display = self.format_value(new, attr, options);
                line.push_str(&format!(" = {} -> {}", old_display, new_display));
            }
            AttributeChangeType::Unchanged => {
                if let Some(ref value) = attr.new_value.as_ref().or(attr.old_value.as_ref()) {
                    let display_value = self.format_value(value, attr, options);
                    line.push_str(&format!(" = {}", display_value));
                }
            }
        }

        // Add markers
        if attr.forces_replacement {
            line.push_str(" # forces replacement");
        }

        line.push('\n');
        line
    }

    /// Format a value for display
    fn format_value(&self, value: &str, attr: &AttributeChange, options: &DiffRenderOptions) -> String {
        // Handle sensitive values
        if attr.sensitive && !options.show_sensitive {
            return "(sensitive)".to_string();
        }

        // Handle computed values
        if attr.computed {
            return "(known after apply)".to_string();
        }

        // Truncate long values
        if value.len() > options.max_value_width {
            let truncated = &value[..options.max_value_width - 3];
            return format!("\"{}...\"", truncated);
        }

        // Quote string values if not already quoted
        if !value.starts_with('"') && !value.starts_with('(') {
            format!("\"{}\"", value)
        } else {
            value.to_string()
        }
    }

    /// Render side-by-side view
    fn render_side_by_side(
        &self,
        resource: &ResourceChange,
        options: &DiffRenderOptions,
    ) -> String {
        let mut output = String::new();
        let half_width = options.terminal_width / 2 - 2;

        let symbol = resource.change_type.symbol();
        let label = resource.change_type.label();
        output.push_str(&format!("{} {} ({})\n", symbol, resource.address, label));

        // Header line
        let old_header = "OLD";
        let new_header = "NEW";
        output.push_str(&format!(
            "    {:<width$} | {}\n",
            old_header,
            new_header,
            width = half_width
        ));
        output.push_str(&format!(
            "    {:-<width$}-+-{:-<width$}\n",
            "",
            "",
            width = half_width
        ));

        // Render each attribute in side-by-side format
        for attr in &resource.attributes {
            if attr.change_type == AttributeChangeType::Unchanged && !options.show_unchanged {
                continue;
            }

            let old_value = attr
                .old_value
                .as_deref()
                .map(|v| self.format_value(v, attr, options))
                .unwrap_or_else(|| "-".to_string());

            let new_value = attr
                .new_value
                .as_deref()
                .map(|v| self.format_value(v, attr, options))
                .unwrap_or_else(|| "-".to_string());

            let symbol = attr.change_type.symbol();

            // Truncate values to fit columns
            let old_display = truncate_str(&old_value, half_width - 4);
            let new_display = truncate_str(&new_value, half_width - 4);

            output.push_str(&format!(
                "  {} {}: {:<width$} | {}\n",
                symbol,
                attr.name,
                old_display,
                new_display,
                width = half_width - attr.name.len() - 4
            ));
        }

        output.push('\n');
        output
    }
}

impl DiffRenderer for AsciiRenderer {
    fn render(&self, plan: &ParsedPlan, options: &DiffRenderOptions) -> String {
        let mut output = String::new();

        // Render summary
        output.push_str(&self.render_summary(&plan.summary));

        // Render each resource
        for resource in &plan.resources {
            if options.side_by_side {
                output.push_str(&self.render_side_by_side(resource, options));
            } else {
                output.push_str(&self.render_resource(resource, options));
            }
        }

        output
    }
}

/// HTML renderer for file export
pub struct HtmlRenderer;

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Generate CSS styles
    fn generate_styles(&self) -> String {
        r#"
<style>
    body {
        font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
        background-color: #1e1e1e;
        color: #d4d4d4;
        padding: 20px;
        line-height: 1.5;
    }
    .summary {
        background-color: #2d2d2d;
        padding: 15px;
        border-radius: 5px;
        margin-bottom: 20px;
    }
    .summary h2 {
        margin: 0 0 10px 0;
        color: #ffffff;
    }
    .summary-item {
        display: inline-block;
        margin-right: 20px;
        padding: 5px 10px;
        border-radius: 3px;
    }
    .resource {
        background-color: #2d2d2d;
        padding: 15px;
        border-radius: 5px;
        margin-bottom: 15px;
        border-left: 4px solid;
    }
    .resource.create { border-left-color: rgb(152, 225, 152); }
    .resource.update { border-left-color: rgb(255, 230, 160); }
    .resource.destroy { border-left-color: rgb(255, 160, 160); }
    .resource.replace { border-left-color: rgb(181, 174, 254); }
    .resource.read { border-left-color: rgb(160, 200, 255); }
    .resource-header {
        font-weight: bold;
        margin-bottom: 10px;
    }
    .attribute {
        padding: 2px 0;
        margin-left: 20px;
    }
    .symbol {
        display: inline-block;
        width: 20px;
        text-align: center;
        font-weight: bold;
    }
    .add { color: rgb(152, 225, 152); }
    .remove { color: rgb(255, 160, 160); }
    .modify { color: rgb(255, 230, 160); }
    .replace { color: rgb(181, 174, 254); }
    .read { color: rgb(160, 200, 255); }
    .unchanged { color: rgb(160, 160, 160); }
    .value { color: #ce9178; }
    .arrow { color: #569cd6; margin: 0 5px; }
    .marker { color: #6a9955; font-style: italic; }
    .attr-name { color: #9cdcfe; }
</style>
"#
        .to_string()
    }

    /// Render summary section
    fn render_summary(&self, summary: &PlanSummary) -> String {
        let mut output = String::new();

        output.push_str("<div class=\"summary\">\n");
        output.push_str("  <h2>Plan Summary</h2>\n");

        if summary.to_add > 0 {
            output.push_str(&format!(
                "  <span class=\"summary-item add\">+{} to add</span>\n",
                summary.to_add
            ));
        }

        if summary.to_change > 0 {
            output.push_str(&format!(
                "  <span class=\"summary-item modify\">~{} to change</span>\n",
                summary.to_change
            ));
        }

        if summary.to_replace > 0 {
            output.push_str(&format!(
                "  <span class=\"summary-item replace\">±{} to replace</span>\n",
                summary.to_replace
            ));
        }

        if summary.to_destroy > 0 {
            output.push_str(&format!(
                "  <span class=\"summary-item remove\">-{} to destroy</span>\n",
                summary.to_destroy
            ));
        }

        if !summary.has_changes() {
            output.push_str("  <span class=\"summary-item unchanged\">No changes</span>\n");
        }

        output.push_str("</div>\n\n");
        output
    }

    /// Render a single resource
    fn render_resource(&self, resource: &ResourceChange, options: &DiffRenderOptions) -> String {
        let mut output = String::new();

        let class = match resource.change_type {
            DiffChangeType::Create => "create",
            DiffChangeType::Update => "update",
            DiffChangeType::Destroy => "destroy",
            DiffChangeType::Replace => "replace",
            DiffChangeType::Read => "read",
            DiffChangeType::NoOp => "unchanged",
        };

        output.push_str(&format!("<div class=\"resource {}\">\n", class));
        output.push_str(&format!(
            "  <div class=\"resource-header\">\n    <span class=\"symbol {}\">{}</span> {} ({})\n  </div>\n",
            class,
            html_escape(resource.change_type.symbol()),
            html_escape(&resource.address),
            resource.change_type.label()
        ));

        // Render attributes
        for attr in &resource.attributes {
            if attr.change_type == AttributeChangeType::Unchanged && !options.show_unchanged {
                continue;
            }

            output.push_str(&self.render_attribute(attr, options));
        }

        output.push_str("</div>\n\n");
        output
    }

    /// Render a single attribute
    fn render_attribute(&self, attr: &AttributeChange, options: &DiffRenderOptions) -> String {
        let mut output = String::new();

        let class = match attr.change_type {
            AttributeChangeType::Added => "add",
            AttributeChangeType::Removed => "remove",
            AttributeChangeType::Modified => "modify",
            AttributeChangeType::Unchanged => "unchanged",
        };

        output.push_str(&format!("  <div class=\"attribute {}\">\n", class));
        output.push_str(&format!(
            "    <span class=\"symbol\">{}</span>",
            html_escape(attr.change_type.symbol())
        ));
        output.push_str(&format!(
            " <span class=\"attr-name\">{}</span>",
            html_escape(&attr.name)
        ));

        // Add value information
        match attr.change_type {
            AttributeChangeType::Added => {
                if let Some(ref value) = attr.new_value {
                    let display = self.format_value(value, attr, options);
                    output.push_str(&format!(
                        " = <span class=\"value\">{}</span>",
                        html_escape(&display)
                    ));
                }
            }
            AttributeChangeType::Removed => {
                if let Some(ref value) = attr.old_value {
                    let display = self.format_value(value, attr, options);
                    output.push_str(&format!(
                        " = <span class=\"value\">{}</span>",
                        html_escape(&display)
                    ));
                }
            }
            AttributeChangeType::Modified => {
                let old = attr.old_value.as_deref().unwrap_or("(unknown)");
                let new = attr.new_value.as_deref().unwrap_or("(unknown)");
                let old_display = self.format_value(old, attr, options);
                let new_display = self.format_value(new, attr, options);
                output.push_str(&format!(
                    " = <span class=\"value\">{}</span><span class=\"arrow\">→</span><span class=\"value\">{}</span>",
                    html_escape(&old_display),
                    html_escape(&new_display)
                ));
            }
            AttributeChangeType::Unchanged => {
                if let Some(ref value) = attr.new_value.as_ref().or(attr.old_value.as_ref()) {
                    let display = self.format_value(value, attr, options);
                    output.push_str(&format!(
                        " = <span class=\"value\">{}</span>",
                        html_escape(&display)
                    ));
                }
            }
        }

        // Add markers
        if attr.forces_replacement {
            output.push_str(" <span class=\"marker\"># forces replacement</span>");
        }

        output.push_str("\n  </div>\n");
        output
    }

    /// Format a value for display
    fn format_value(&self, value: &str, attr: &AttributeChange, options: &DiffRenderOptions) -> String {
        if attr.sensitive && !options.show_sensitive {
            return "(sensitive)".to_string();
        }

        if attr.computed {
            return "(known after apply)".to_string();
        }

        if value.len() > options.max_value_width {
            let truncated = &value[..options.max_value_width - 3];
            return format!("\"{}...\"", truncated);
        }

        if !value.starts_with('"') && !value.starts_with('(') {
            format!("\"{}\"", value)
        } else {
            value.to_string()
        }
    }
}

impl DiffRenderer for HtmlRenderer {
    fn render(&self, plan: &ParsedPlan, options: &DiffRenderOptions) -> String {
        let mut output = String::new();

        output.push_str("<!DOCTYPE html>\n");
        output.push_str("<html lang=\"en\">\n");
        output.push_str("<head>\n");
        output.push_str("  <meta charset=\"UTF-8\">\n");
        output.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        output.push_str("  <title>Plan Diff</title>\n");
        output.push_str(&self.generate_styles());
        output.push_str("</head>\n");
        output.push_str("<body>\n\n");

        // Render summary
        output.push_str(&self.render_summary(&plan.summary));

        // Render resources
        for resource in &plan.resources {
            output.push_str(&self.render_resource(resource, options));
        }

        output.push_str("</body>\n");
        output.push_str("</html>\n");

        output
    }
}

/// Helper function to truncate a string to a maximum length
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

/// Helper function to escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::types::ResourceChange;

    fn sample_plan() -> ParsedPlan {
        let mut plan = ParsedPlan::new();

        let mut create_resource = ResourceChange::new("aws_instance.web", DiffChangeType::Create);
        create_resource.add_attribute(
            AttributeChange::new("ami", AttributeChangeType::Added)
                .with_new_value("ami-12345678"),
        );
        create_resource.add_attribute(
            AttributeChange::new("instance_type", AttributeChangeType::Added)
                .with_new_value("t3.micro"),
        );
        plan.add_resource(create_resource);

        let mut update_resource =
            ResourceChange::new("aws_security_group.main", DiffChangeType::Update);
        update_resource.add_attribute(
            AttributeChange::new("ingress.0.from_port", AttributeChangeType::Modified)
                .with_old_value("80")
                .with_new_value("443"),
        );
        plan.add_resource(update_resource);

        let mut destroy_resource =
            ResourceChange::new("aws_instance.old", DiffChangeType::Destroy);
        destroy_resource.add_attribute(
            AttributeChange::new("ami", AttributeChangeType::Removed)
                .with_old_value("ami-old12345"),
        );
        plan.add_resource(destroy_resource);

        plan
    }

    #[test]
    fn test_ascii_renderer_summary() {
        let renderer = AsciiRenderer::new();
        let plan = sample_plan();
        let options = DiffRenderOptions::default();

        let output = renderer.render(&plan, &options);

        assert!(output.contains("Plan Summary:"));
        assert!(output.contains("+1 to add"));
        assert!(output.contains("~1 to change"));
        assert!(output.contains("-1 to destroy"));
    }

    #[test]
    fn test_ascii_renderer_resources() {
        let renderer = AsciiRenderer::new();
        let plan = sample_plan();
        let options = DiffRenderOptions::default();

        let output = renderer.render(&plan, &options);

        assert!(output.contains("+ aws_instance.web (will be created)"));
        assert!(output.contains("~ aws_security_group.main (will be updated)"));
        assert!(output.contains("- aws_instance.old (will be destroyed)"));
    }

    #[test]
    fn test_ascii_renderer_attributes() {
        let renderer = AsciiRenderer::new();
        let plan = sample_plan();
        let options = DiffRenderOptions::default();

        let output = renderer.render(&plan, &options);

        assert!(output.contains("+ ami = \"ami-12345678\""));
        assert!(output.contains("~ ingress.0.from_port = \"80\" -> \"443\""));
        assert!(output.contains("- ami = \"ami-old12345\""));
    }

    #[test]
    fn test_html_renderer_structure() {
        let renderer = HtmlRenderer::new();
        let plan = sample_plan();
        let options = DiffRenderOptions::default();

        let output = renderer.render(&plan, &options);

        assert!(output.contains("<!DOCTYPE html>"));
        assert!(output.contains("<html lang=\"en\">"));
        assert!(output.contains("<style>"));
        assert!(output.contains("class=\"resource create\""));
        assert!(output.contains("class=\"resource update\""));
        assert!(output.contains("class=\"resource destroy\""));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"test\""), "&quot;test&quot;");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a long string", 10), "this is...");
        assert_eq!(truncate_str("ab", 5), "ab");
    }

    #[test]
    fn test_sensitive_value_hidden() {
        let renderer = AsciiRenderer::new();
        let mut plan = ParsedPlan::new();

        let mut resource = ResourceChange::new("aws_db.main", DiffChangeType::Create);
        resource.add_attribute(
            AttributeChange::new("password", AttributeChangeType::Added)
                .with_new_value("secret123")
                .with_sensitive(true),
        );
        plan.add_resource(resource);

        let options = DiffRenderOptions {
            show_sensitive: false,
            ..Default::default()
        };

        let output = renderer.render(&plan, &options);
        assert!(output.contains("(sensitive)"));
        assert!(!output.contains("secret123"));
    }

    #[test]
    fn test_sensitive_value_shown() {
        let renderer = AsciiRenderer::new();
        let mut plan = ParsedPlan::new();

        let mut resource = ResourceChange::new("aws_db.main", DiffChangeType::Create);
        resource.add_attribute(
            AttributeChange::new("password", AttributeChangeType::Added)
                .with_new_value("secret123")
                .with_sensitive(true),
        );
        plan.add_resource(resource);

        let options = DiffRenderOptions {
            show_sensitive: true,
            ..Default::default()
        };

        let output = renderer.render(&plan, &options);
        assert!(output.contains("secret123"));
    }

    #[test]
    fn test_side_by_side_view() {
        let renderer = AsciiRenderer::new();
        let plan = sample_plan();
        let options = DiffRenderOptions {
            side_by_side: true,
            terminal_width: 100,
            ..Default::default()
        };

        let output = renderer.render(&plan, &options);

        assert!(output.contains("OLD"));
        assert!(output.contains("NEW"));
        assert!(output.contains("|"));
    }
}
