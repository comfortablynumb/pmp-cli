use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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
        infrastructure_root: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<crate::commands::tags::TagConfig> {
        let tags_file = infrastructure_root.join(".pmp").join("tags").join(format!(
            "{}-{}.json",
            resource.metadata.name, resource.metadata.environment_name
        ));

        if !tags_file.exists() {
            anyhow::bail!("No tags found");
        }

        let content = std::fs::read_to_string(&tags_file)?;
        let config: crate::commands::tags::TagConfig = serde_json::from_str(&content)?;

        Ok(config)
    }

    fn search_terraform_resources(
        _ctx: &Context,
        env_path: &Path,
        resource_type: Option<&str>,
        resource_name: Option<&str>,
    ) -> Result<Vec<Match>> {
        let mut matches = Vec::new();

        // In a real implementation:
        // 1. Parse Terraform files (.tf)
        // 2. Extract resource definitions
        // 3. Match against criteria

        // Mock implementation - search in .tf files
        for entry in std::fs::read_dir(env_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("tf")
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                for line in content.lines() {
                    if let Some(rtype) = resource_type
                        && line.contains(&format!("resource \"{}\"", rtype))
                    {
                        matches.push(Match {
                            field: "resource_type".to_string(),
                            value: rtype.to_string(),
                            context: Some(line.trim().to_string()),
                        });
                    }

                    if let Some(rname) = resource_name
                        && line.contains(rname)
                    {
                        matches.push(Match {
                            field: "resource_name".to_string(),
                            value: rname.to_string(),
                            context: Some(line.trim().to_string()),
                        });
                    }
                }
            }
        }

        Ok(matches)
    }

    fn search_terraform_outputs(
        _ctx: &Context,
        env_path: &Path,
        output_name: &str,
    ) -> Result<Vec<Match>> {
        let mut matches = Vec::new();

        // Search in .tf files for output definitions
        for entry in std::fs::read_dir(env_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("tf")
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                for line in content.lines() {
                    if line.contains(&format!("output \"{}\"", output_name)) {
                        matches.push(Match {
                            field: "output".to_string(),
                            value: output_name.to_string(),
                            context: Some(line.trim().to_string()),
                        });
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
}
