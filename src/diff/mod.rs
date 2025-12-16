//! Plan diff visualization module
//!
//! This module provides parsing and rendering capabilities for OpenTofu/Terraform
//! plan output, displaying changes in a formatted, color-coded manner.
//!
//! # Features
//!
//! - **Parsing**: Extract resource and attribute changes from plan output
//! - **ASCII Rendering**: Terminal-friendly colored diff output
//! - **HTML Rendering**: Export diffs to HTML for documentation/sharing
//! - **Side-by-side view**: Optional two-column comparison
//!
//! # Example
//!
//! ```ignore
//! use pmp::diff::{PlanParser, AsciiRenderer, DiffRenderer, DiffRenderOptions};
//!
//! let parser = PlanParser::new();
//! let plan = parser.parse(&plan_output)?;
//!
//! let renderer = AsciiRenderer::new();
//! let options = DiffRenderOptions::default();
//! let diff_output = renderer.render(&plan, &options);
//!
//! println!("{}", diff_output);
//! ```

mod parser;
mod renderer;
mod types;

pub use parser::PlanParser;
pub use renderer::{AsciiRenderer, DiffRenderer, HtmlRenderer};
pub use types::{
    AttributeChange, AttributeChangeType, DiffChangeType, DiffRenderOptions, ParsedPlan,
    PlanSummary, ResourceChange,
};
