use std::collections::HashMap;

use crate::template::metadata::TemplateResource;

use super::analyzer::{ResourceInfo, StateAnalysis};

/// Matches imported resources against available templates
pub struct TemplateMatcher {
    templates: Vec<TemplateResource>,
}

impl TemplateMatcher {
    pub fn new(templates: Vec<TemplateResource>) -> Self {
        Self { templates }
    }

    /// Find template matches for the given state analysis
    pub fn find_matches(&self, analysis: &StateAnalysis) -> Vec<TemplateMatch> {
        let mut matches = Vec::new();

        for template in &self.templates {
            let similarity = self.calculate_similarity(template, &analysis.resources);

            if similarity > 0.5 {
                // Only include matches with >50% confidence
                let match_details = self.get_match_details(template, &analysis.resources);

                matches.push(TemplateMatch {
                    template_pack: "".to_string(), // TODO: Get from template
                    template_name: template.metadata.name.clone(),
                    confidence: similarity,
                    matching_resources: match_details.matching,
                    missing_resources: match_details.missing,
                    extra_resources: match_details.extra,
                });
            }
        }

        // Sort by confidence (highest first)
        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matches
    }

    /// Calculate similarity score between template and resources
    fn calculate_similarity(
        &self,
        _template: &TemplateResource,
        resources: &[ResourceInfo],
    ) -> f64 {
        // For now, return a simple score
        // TODO: Implement proper similarity calculation based on:
        // - Resource types match
        // - Resource count match
        // - Required resources present
        // - Provider compatibility

        if resources.is_empty() {
            return 0.0;
        }

        // Placeholder logic
        0.75
    }

    /// Get detailed match information
    fn get_match_details(
        &self,
        _template: &TemplateResource,
        resources: &[ResourceInfo],
    ) -> MatchDetails {
        // For now, return simple details
        // TODO: Implement proper matching logic

        let matching: Vec<String> = resources.iter().map(|r| r.address.clone()).collect();

        MatchDetails {
            matching,
            missing: Vec::new(),
            extra: Vec::new(),
        }
    }

    /// Score a specific resource type match
    fn score_resource_type(&self, expected: &str, actual: &str) -> f64 {
        if expected == actual {
            1.0
        } else if expected.starts_with(actual) || actual.starts_with(expected) {
            0.7
        } else {
            0.0
        }
    }

    /// Calculate resource count similarity
    fn calculate_count_similarity(&self, expected: usize, actual: usize) -> f64 {
        if expected == actual {
            1.0
        } else if expected == 0 || actual == 0 {
            0.0
        } else {
            let diff = (expected as f64 - actual as f64).abs();
            let max = expected.max(actual) as f64;
            1.0 - (diff / max)
        }
    }
}

/// Template match result
#[derive(Debug, Clone)]
pub struct TemplateMatch {
    pub template_pack: String,
    pub template_name: String,
    pub confidence: f64,
    pub matching_resources: Vec<String>,
    pub missing_resources: Vec<String>,
    pub extra_resources: Vec<String>,
}

/// Match details
struct MatchDetails {
    matching: Vec<String>,
    missing: Vec<String>,
    extra: Vec<String>,
}

/// Resource type mapping for common aliases
pub struct ResourceTypeMapper;

impl ResourceTypeMapper {
    /// Get canonical resource type name
    pub fn get_canonical_type(resource_type: &str) -> String {
        // Map common aliases to canonical names
        let mappings: HashMap<&str, &str> = [
            ("aws_instance", "aws_instance"),
            ("aws_vpc", "aws_vpc"),
            ("aws_subnet", "aws_subnet"),
            ("aws_security_group", "aws_security_group"),
            ("aws_lb", "aws_lb"),
            ("aws_alb", "aws_lb"), // ALB is an alias for LB
        ]
        .iter()
        .copied()
        .collect();

        mappings
            .get(resource_type)
            .unwrap_or(&resource_type)
            .to_string()
    }

    /// Check if two resource types are compatible
    pub fn are_compatible(type1: &str, type2: &str) -> bool {
        let canonical1 = Self::get_canonical_type(type1);
        let canonical2 = Self::get_canonical_type(type2);

        canonical1 == canonical2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_type_mapper() {
        assert_eq!(
            ResourceTypeMapper::get_canonical_type("aws_instance"),
            "aws_instance"
        );
        assert_eq!(ResourceTypeMapper::get_canonical_type("aws_alb"), "aws_lb");
        assert!(ResourceTypeMapper::are_compatible("aws_alb", "aws_lb"));
        assert!(ResourceTypeMapper::are_compatible("aws_vpc", "aws_vpc"));
        assert!(!ResourceTypeMapper::are_compatible("aws_vpc", "aws_subnet"));
    }
}
