use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// Stub for removed tags functionality
#[derive(Debug, Serialize, Deserialize)]
struct TagConfig {
    tags: HashMap<String, String>,
}

pub struct SearchCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub project: String,
    pub environment: String,
    pub match_type: MatchType,
    pub matches: Vec<Match>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MatchType {
    Tag,
    Resource,
    Name,
    Output,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Match {
    pub field: String,
    pub value: String,
    pub context: Option<String>,
}

impl SearchCommand {
    pub fn execute_by_tags(ctx: &Context, tag_filters: Vec<String>) -> Result<()> {
        ctx.output.section("Search by Tags");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Parse tag filters (KEY=VALUE or just KEY)
        let mut filter_map: HashMap<String, Option<String>> = HashMap::new();
        for filter in &tag_filters {
            let parts: Vec<&str> = filter.splitn(2, '=').collect();
            if parts.len() == 2 {
                filter_map.insert(parts[0].to_string(), Some(parts[1].to_string()));
            } else {
                filter_map.insert(parts[0].to_string(), None);
            }
        }

        ctx.output.subsection("Search Criteria");
        for (key, value) in &filter_map {
            if let Some(v) = value {
                ctx.output.dimmed(&format!("{} = {}", key, v));
            } else {
                ctx.output.dimmed(&format!("{} (any value)", key));
            }
        }
        output::blank();

        // Search projects
        let projects = crate::collection::CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        let mut results = Vec::new();

        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if !ctx.fs.exists(&environments_dir) {
                continue;
            }

            for env_entry in ctx.fs.read_dir(&environments_dir)? {
                if !ctx.fs.is_dir(&env_entry) {
                    continue;
                }

                let env_file = env_entry.join(".pmp.environment.yaml");
                if !ctx.fs.exists(&env_file) {
                    continue;
                }

                let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

                // Load tags
                if let Ok(tag_config) = Self::load_tags(ctx, &infrastructure_root, &resource) {
                    let mut matches = Vec::new();
                    let mut all_match = true;

                    for (filter_key, filter_value) in &filter_map {
                        if let Some(tag_value) = tag_config.tags.get(filter_key) {
                            if let Some(expected_value) = filter_value {
                                if tag_value == expected_value {
                                    matches.push(Match {
                                        field: filter_key.clone(),
                                        value: tag_value.clone(),
                                        context: None,
                                    });
                                } else {
                                    all_match = false;
                                    break;
                                }
                            } else {
                                // Just checking for key existence
                                matches.push(Match {
                                    field: filter_key.clone(),
                                    value: tag_value.clone(),
                                    context: None,
                                });
                            }
                        } else {
                            all_match = false;
                            break;
                        }
                    }

                    if all_match && !matches.is_empty() {
                        results.push(SearchResult {
                            project: resource.metadata.name.clone(),
                            environment: resource.metadata.environment_name.clone(),
                            match_type: MatchType::Tag,
                            matches,
                        });
                    }
                }
            }
        }

        // Display results
        Self::display_search_results(ctx, &results)?;

        Ok(())
    }

    pub fn execute_by_resources(
        ctx: &Context,
        resource_type: Option<&str>,
        resource_name: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Search by Resources");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output.subsection("Search Criteria");
        if let Some(rtype) = resource_type {
            ctx.output.dimmed(&format!("Resource type: {}", rtype));
        }
        if let Some(rname) = resource_name {
            ctx.output.dimmed(&format!("Resource name: {}", rname));
        }
        output::blank();

        // Search projects
        let projects = crate::collection::CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        let mut results = Vec::new();

        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if !ctx.fs.exists(&environments_dir) {
                continue;
            }

            for env_entry in ctx.fs.read_dir(&environments_dir)? {
                if !ctx.fs.is_dir(&env_entry) {
                    continue;
                }

                let env_file = env_entry.join(".pmp.environment.yaml");
                if !ctx.fs.exists(&env_file) {
                    continue;
                }

                let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

                // Search in Terraform files
                let matches = Self::search_terraform_resources(
                    ctx,
                    &env_entry,
                    resource_type,
                    resource_name,
                )?;

                if !matches.is_empty() {
                    results.push(SearchResult {
                        project: resource.metadata.name.clone(),
                        environment: resource.metadata.environment_name.clone(),
                        match_type: MatchType::Resource,
                        matches,
                    });
                }
            }
        }

        // Display results
        Self::display_search_results(ctx, &results)?;

        Ok(())
    }

    pub fn execute_by_name(ctx: &Context, pattern: &str) -> Result<()> {
        ctx.output.section("Search by Name");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output.subsection("Search Criteria");
        ctx.output.dimmed(&format!("Pattern: {}", pattern));
        output::blank();

        // Search projects
        let projects = crate::collection::CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        let mut results = Vec::new();

        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if !ctx.fs.exists(&environments_dir) {
                continue;
            }

            for env_entry in ctx.fs.read_dir(&environments_dir)? {
                if !ctx.fs.is_dir(&env_entry) {
                    continue;
                }

                let env_file = env_entry.join(".pmp.environment.yaml");
                if !ctx.fs.exists(&env_file) {
                    continue;
                }

                let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

                // Check project name
                if resource.metadata.name.contains(pattern) {
                    results.push(SearchResult {
                        project: resource.metadata.name.clone(),
                        environment: resource.metadata.environment_name.clone(),
                        match_type: MatchType::Name,
                        matches: vec![Match {
                            field: "project".to_string(),
                            value: resource.metadata.name.clone(),
                            context: None,
                        }],
                    });
                }

                // Check environment name
                if resource.metadata.environment_name.contains(pattern)
                    && !resource.metadata.name.contains(pattern)
                {
                    results.push(SearchResult {
                        project: resource.metadata.name.clone(),
                        environment: resource.metadata.environment_name.clone(),
                        match_type: MatchType::Name,
                        matches: vec![Match {
                            field: "environment".to_string(),
                            value: resource.metadata.environment_name.clone(),
                            context: None,
                        }],
                    });
                }
            }
        }

        // Display results
        Self::display_search_results(ctx, &results)?;

        Ok(())
    }

    pub fn execute_by_output(ctx: &Context, output_name: &str) -> Result<()> {
        ctx.output.section("Search by Output");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output.subsection("Search Criteria");
        ctx.output.dimmed(&format!("Output name: {}", output_name));
        output::blank();

        // Search projects
        let projects = crate::collection::CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        let mut results = Vec::new();

        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if !ctx.fs.exists(&environments_dir) {
                continue;
            }

            for env_entry in ctx.fs.read_dir(&environments_dir)? {
                if !ctx.fs.is_dir(&env_entry) {
                    continue;
                }

                let env_file = env_entry.join(".pmp.environment.yaml");
                if !ctx.fs.exists(&env_file) {
                    continue;
                }

                let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

                // Search for outputs in Terraform files
                let matches = Self::search_terraform_outputs(ctx, &env_entry, output_name)?;

                if !matches.is_empty() {
                    results.push(SearchResult {
                        project: resource.metadata.name.clone(),
                        environment: resource.metadata.environment_name.clone(),
                        match_type: MatchType::Output,
                        matches,
                    });
                }
            }
        }

        // Display results
        Self::display_search_results(ctx, &results)?;

        Ok(())
    }

    // Helper functions

    fn load_tags(
        _ctx: &Context,
        _infrastructure_root: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<TagConfig> {
        // Extract tags from inputs
        // Tags are stored in inputs with "tag_" prefix (e.g., tag_environment, tag_owner)
        let mut tags = HashMap::new();

        for (key, value) in &resource.spec.inputs {
            if key.starts_with("tag_") {
                let tag_name = key.strip_prefix("tag_").unwrap();
                let tag_value = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => value.to_string(),
                };
                tags.insert(tag_name.to_string(), tag_value);
            }
        }

        Ok(TagConfig { tags })
    }

    fn search_terraform_resources(
        ctx: &Context,
        env_path: &Path,
        resource_type: Option<&str>,
        resource_name: Option<&str>,
    ) -> Result<Vec<Match>> {
        let mut matches = Vec::new();

        // Parse Terraform files (.tf) to find resource definitions
        // Format: resource "type" "name" { ... }
        let resource_regex = regex::Regex::new(r#"resource\s+"([^"]+)"\s+"([^"]+)"\s*\{"#).unwrap();

        for path in ctx.fs.read_dir(env_path)? {
            if path.extension().and_then(|s| s.to_str()) == Some("tf")
                && let Ok(content) = ctx.fs.read_to_string(&path)
            {
                for (line_num, line) in content.lines().enumerate() {
                    if let Some(captures) = resource_regex.captures(line) {
                        let res_type = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                        let res_name = captures.get(2).map(|m| m.as_str()).unwrap_or("");

                        // Check if type matches (if specified)
                        let type_matches =
                            resource_type.is_none() || resource_type == Some(res_type);

                        // Check if name matches (if specified)
                        let name_matches = resource_name.is_none()
                            || resource_name.map(|n| res_name.contains(n)).unwrap_or(false);

                        if type_matches && name_matches {
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown");

                            matches.push(Match {
                                field: "resource".to_string(),
                                value: format!("{}.{}", res_type, res_name),
                                context: Some(format!(
                                    "{}:{} - {}",
                                    file_name,
                                    line_num + 1,
                                    line.trim()
                                )),
                            });
                        }
                    }
                }
            }
        }

        Ok(matches)
    }

    fn search_terraform_outputs(
        ctx: &Context,
        env_path: &Path,
        output_name: &str,
    ) -> Result<Vec<Match>> {
        let mut matches = Vec::new();

        // Parse Terraform files (.tf) to find output definitions
        // Format: output "name" { ... }
        let output_regex = regex::Regex::new(r#"output\s+"([^"]+)"\s*\{"#).unwrap();

        for path in ctx.fs.read_dir(env_path)? {
            if path.extension().and_then(|s| s.to_str()) == Some("tf")
                && let Ok(content) = ctx.fs.read_to_string(&path)
            {
                for (line_num, line) in content.lines().enumerate() {
                    if let Some(captures) = output_regex.captures(line) {
                        let out_name = captures.get(1).map(|m| m.as_str()).unwrap_or("");

                        // Check if output name matches (exact or contains)
                        if out_name == output_name || out_name.contains(output_name) {
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown");

                            matches.push(Match {
                                field: "output".to_string(),
                                value: out_name.to_string(),
                                context: Some(format!(
                                    "{}:{} - {}",
                                    file_name,
                                    line_num + 1,
                                    line.trim()
                                )),
                            });
                        }
                    }
                }
            }
        }

        Ok(matches)
    }

    fn display_search_results(ctx: &Context, results: &[SearchResult]) -> Result<()> {
        if results.is_empty() {
            ctx.output.info("No matches found");
            return Ok(());
        }

        ctx.output.subsection("Results");
        output::blank();

        for result in results {
            ctx.output
                .dimmed(&format!("{}/{}", result.project, result.environment));

            for m in &result.matches {
                ctx.output.dimmed(&format!("  {}: {}", m.field, m.value));
                if let Some(context) = &m.context {
                    ctx.output.dimmed(&format!("    {}", context));
                }
            }

            output::blank();
        }

        ctx.output
            .success(&format!("{} matches found", results.len()));

        Ok(())
    }

    /// Parse tag filters from CLI input
    #[cfg(test)]
    fn parse_tag_filters(filters: &[String]) -> HashMap<String, Option<String>> {
        let mut filter_map: HashMap<String, Option<String>> = HashMap::new();

        for filter in filters {
            let parts: Vec<&str> = filter.splitn(2, '=').collect();
            if parts.len() == 2 {
                filter_map.insert(parts[0].to_string(), Some(parts[1].to_string()));
            } else {
                filter_map.insert(parts[0].to_string(), None);
            }
        }

        filter_map
    }

    /// Check if tags match filter criteria
    #[cfg(test)]
    fn tags_match_filters(
        tags: &HashMap<String, String>,
        filters: &HashMap<String, Option<String>>,
    ) -> bool {
        for (filter_key, filter_value) in filters {
            match tags.get(filter_key) {
                Some(tag_value) => {
                    if let Some(expected_value) = filter_value {
                        if tag_value != expected_value {
                            return false;
                        }
                    }
                }
                None => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tag_filters_key_value() {
        let filters = vec!["env=production".to_string()];
        let result = SearchCommand::parse_tag_filters(&filters);

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("env"), Some(&Some("production".to_string())));
    }

    #[test]
    fn test_parse_tag_filters_key_only() {
        let filters = vec!["monitored".to_string()];
        let result = SearchCommand::parse_tag_filters(&filters);

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("monitored"), Some(&None));
    }

    #[test]
    fn test_parse_tag_filters_mixed() {
        let filters = vec![
            "env=production".to_string(),
            "team=platform".to_string(),
            "critical".to_string(),
        ];
        let result = SearchCommand::parse_tag_filters(&filters);

        assert_eq!(result.len(), 3);
        assert_eq!(result.get("env"), Some(&Some("production".to_string())));
        assert_eq!(result.get("team"), Some(&Some("platform".to_string())));
        assert_eq!(result.get("critical"), Some(&None));
    }

    #[test]
    fn test_parse_tag_filters_value_with_equals() {
        let filters = vec!["url=https://example.com?foo=bar".to_string()];
        let result = SearchCommand::parse_tag_filters(&filters);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("url"),
            Some(&Some("https://example.com?foo=bar".to_string()))
        );
    }

    #[test]
    fn test_tags_match_filters_exact_match() {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "production".to_string());
        tags.insert("team".to_string(), "platform".to_string());

        let mut filters = HashMap::new();
        filters.insert("env".to_string(), Some("production".to_string()));

        assert!(SearchCommand::tags_match_filters(&tags, &filters));
    }

    #[test]
    fn test_tags_match_filters_key_exists() {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "production".to_string());
        tags.insert("monitored".to_string(), "true".to_string());

        let mut filters = HashMap::new();
        filters.insert("monitored".to_string(), None);

        assert!(SearchCommand::tags_match_filters(&tags, &filters));
    }

    #[test]
    fn test_tags_match_filters_value_mismatch() {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "staging".to_string());

        let mut filters = HashMap::new();
        filters.insert("env".to_string(), Some("production".to_string()));

        assert!(!SearchCommand::tags_match_filters(&tags, &filters));
    }

    #[test]
    fn test_tags_match_filters_key_missing() {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "production".to_string());

        let mut filters = HashMap::new();
        filters.insert("team".to_string(), Some("platform".to_string()));

        assert!(!SearchCommand::tags_match_filters(&tags, &filters));
    }

    #[test]
    fn test_tags_match_filters_multiple_conditions() {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "production".to_string());
        tags.insert("team".to_string(), "platform".to_string());
        tags.insert("critical".to_string(), "true".to_string());

        let mut filters = HashMap::new();
        filters.insert("env".to_string(), Some("production".to_string()));
        filters.insert("team".to_string(), Some("platform".to_string()));
        filters.insert("critical".to_string(), None);

        assert!(SearchCommand::tags_match_filters(&tags, &filters));
    }

    #[test]
    fn test_tags_match_filters_empty_filters() {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "production".to_string());

        let filters: HashMap<String, Option<String>> = HashMap::new();

        assert!(SearchCommand::tags_match_filters(&tags, &filters));
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            project: "my-vpc".to_string(),
            environment: "prod".to_string(),
            match_type: MatchType::Tag,
            matches: vec![Match {
                field: "env".to_string(),
                value: "production".to_string(),
                context: None,
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("my-vpc"));
        assert!(json.contains("prod"));
        assert!(json.contains("Tag"));
    }

    #[test]
    fn test_match_type_variants() {
        let tag = MatchType::Tag;
        let resource = MatchType::Resource;
        let name = MatchType::Name;
        let output = MatchType::Output;

        let tag_json = serde_json::to_string(&tag).unwrap();
        let resource_json = serde_json::to_string(&resource).unwrap();
        let name_json = serde_json::to_string(&name).unwrap();
        let output_json = serde_json::to_string(&output).unwrap();

        assert_eq!(tag_json, "\"Tag\"");
        assert_eq!(resource_json, "\"Resource\"");
        assert_eq!(name_json, "\"Name\"");
        assert_eq!(output_json, "\"Output\"");
    }
}
