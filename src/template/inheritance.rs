use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::discovery::{TemplateDiscovery, TemplateInfo, TemplatePackInfo};
use super::metadata::{
    InputDefinition, TemplateExtendsRef, TemplateResource, TemplateSpec,
};

/// A resolved template with inheritance chain applied
#[derive(Debug, Clone)]
pub struct ResolvedTemplate {
    /// The merged template resource (base + child, child wins)
    pub resource: TemplateResource,
    /// Path to the child template directory (for rendering)
    pub path: PathBuf,
    /// All template paths in inheritance order [base, ..., child]
    /// Used to render files from base to child
    pub base_paths: Vec<PathBuf>,
    /// Inheritance chain for debugging (e.g., ["base-pack/base", "child-pack/child"])
    pub inheritance_chain: Vec<String>,
    /// Version of the resolved template
    pub version: Option<semver::Version>,
}

/// Resolves template inheritance chains
pub struct TemplateResolver;

impl TemplateResolver {
    /// Resolve a template's inheritance chain and merge specs
    /// Returns a ResolvedTemplate with merged inputs, dependencies, etc.
    pub fn resolve(
        fs: &dyn crate::traits::FileSystem,
        output: &dyn crate::traits::Output,
        template: &TemplateInfo,
        template_pack: &TemplatePackInfo,
        all_template_packs: &[TemplatePackInfo],
    ) -> Result<ResolvedTemplate> {
        let mut visited = HashSet::new();
        Self::resolve_recursive(
            fs,
            output,
            template,
            template_pack,
            all_template_packs,
            &mut visited,
        )
    }

    fn resolve_recursive(
        fs: &dyn crate::traits::FileSystem,
        output: &dyn crate::traits::Output,
        template: &TemplateInfo,
        template_pack: &TemplatePackInfo,
        all_template_packs: &[TemplatePackInfo],
        visited: &mut HashSet<String>,
    ) -> Result<ResolvedTemplate> {
        // Create unique identifier for cycle detection
        let template_id = format!(
            "{}/{}@{}",
            template_pack.resource.metadata.name,
            template.resource.metadata.name,
            template.version.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "0.0.1".to_string())
        );

        if visited.contains(&template_id) {
            anyhow::bail!(
                "Circular template inheritance detected: {}",
                template_id
            );
        }

        visited.insert(template_id.clone());

        // Check if this template extends another
        if let Some(extends) = &template.resource.spec.extends {
            // Find the base template
            let (base_template, base_pack) =
                Self::find_template(fs, output, extends, all_template_packs)?;

            // Recursively resolve the base template
            let base_resolved = Self::resolve_recursive(
                fs,
                output,
                &base_template,
                &base_pack,
                all_template_packs,
                visited,
            )?;

            // Merge base and child (child wins)
            let merged_resource = Self::merge_templates(&base_resolved.resource, &template.resource);

            // Build inheritance chain
            let mut base_paths = base_resolved.base_paths;
            base_paths.push(template.path.clone());

            let mut inheritance_chain = base_resolved.inheritance_chain;
            inheritance_chain.push(template_id);

            Ok(ResolvedTemplate {
                resource: merged_resource,
                path: template.path.clone(),
                base_paths,
                inheritance_chain,
                version: template.version.clone(),
            })
        } else {
            // No inheritance, return the template as-is
            Ok(ResolvedTemplate {
                resource: template.resource.clone(),
                path: template.path.clone(),
                base_paths: vec![template.path.clone()],
                inheritance_chain: vec![template_id],
                version: template.version.clone(),
            })
        }
    }

    /// Find a template by its extends reference
    fn find_template(
        fs: &dyn crate::traits::FileSystem,
        output: &dyn crate::traits::Output,
        extends: &TemplateExtendsRef,
        all_template_packs: &[TemplatePackInfo],
    ) -> Result<(TemplateInfo, TemplatePackInfo)> {
        // Find the template pack
        let pack = all_template_packs
            .iter()
            .find(|p| p.resource.metadata.name == extends.template_pack)
            .with_context(|| {
                format!(
                    "Base template pack '{}' not found",
                    extends.template_pack
                )
            })?;

        // Discover templates in the pack
        let templates = TemplateDiscovery::discover_templates_in_pack(fs, output, &pack.path)?;

        // Find templates with matching name
        let matching: Vec<_> = templates
            .into_iter()
            .filter(|t| t.resource.metadata.name == extends.template)
            .collect();

        if matching.is_empty() {
            anyhow::bail!(
                "Base template '{}' not found in pack '{}'",
                extends.template,
                extends.template_pack
            );
        }

        // If version is specified, find exact match
        if let Some(version_str) = &extends.version {
            let target_version = semver::Version::parse(version_str).with_context(|| {
                format!("Invalid version '{}' in extends reference", version_str)
            })?;

            let template = matching
                .into_iter()
                .find(|t| t.version.as_ref() == Some(&target_version))
                .with_context(|| {
                    format!(
                        "Base template '{}' version '{}' not found in pack '{}'",
                        extends.template, version_str, extends.template_pack
                    )
                })?;

            return Ok((template, pack.clone()));
        }

        // No version specified, use the latest (first one, since they're sorted descending)
        let template = matching.into_iter().next().unwrap();
        Ok((template, pack.clone()))
    }

    /// Merge two template specs (child wins on conflicts)
    fn merge_templates(base: &TemplateResource, child: &TemplateResource) -> TemplateResource {
        TemplateResource {
            api_version: child.api_version.clone(),
            kind: child.kind.clone(),
            metadata: child.metadata.clone(),
            spec: Self::merge_specs(&base.spec, &child.spec),
        }
    }

    /// Merge two template specs (child wins on conflicts)
    fn merge_specs(base: &TemplateSpec, child: &TemplateSpec) -> TemplateSpec {
        TemplateSpec {
            // Child always wins for these
            extends: None, // Clear extends after resolving
            api_version: child.api_version.clone(),
            kind: child.kind.clone(),
            executor: child.executor.clone(),
            order: child.order,
            plugins: child.plugins.clone().or_else(|| base.plugins.clone()),
            projects: child.projects.clone(),
            hooks: Self::merge_hooks(&base.hooks, &child.hooks),

            // Merge inputs (child overrides same-name inputs)
            inputs: Self::merge_inputs(&base.inputs, &child.inputs),

            // Merge environment overrides (child overrides same-name envs)
            environments: Self::merge_environments(&base.environments, &child.environments),

            // Concatenate dependencies
            dependencies: Self::merge_dependencies(&base.dependencies, &child.dependencies),
        }
    }

    /// Merge inputs - child inputs override base inputs with same name
    fn merge_inputs(base: &[InputDefinition], child: &[InputDefinition]) -> Vec<InputDefinition> {
        let mut merged = base.to_vec();

        for child_input in child {
            if let Some(pos) = merged.iter().position(|i| i.name == child_input.name) {
                // Child overrides base input with same name
                merged[pos] = child_input.clone();
            } else {
                // Add new input from child
                merged.push(child_input.clone());
            }
        }

        merged
    }

    /// Merge environment overrides - child overrides win
    fn merge_environments(
        base: &HashMap<String, super::metadata::EnvironmentOverrides>,
        child: &HashMap<String, super::metadata::EnvironmentOverrides>,
    ) -> HashMap<String, super::metadata::EnvironmentOverrides> {
        let mut merged = base.clone();

        for (env_name, child_overrides) in child {
            if let Some(base_overrides) = merged.get_mut(env_name) {
                // Merge input overrides for this environment
                // Child inputs override base inputs with same name
                let merged_inputs =
                    Self::merge_inputs(&base_overrides.overrides.inputs, &child_overrides.overrides.inputs);
                base_overrides.overrides.inputs = merged_inputs;
            } else {
                // Add new environment from child
                merged.insert(env_name.clone(), child_overrides.clone());
            }
        }

        merged
    }

    /// Merge dependencies - concatenate, removing duplicates by dependency_name
    fn merge_dependencies(
        base: &[super::metadata::TemplateDependency],
        child: &[super::metadata::TemplateDependency],
    ) -> Vec<super::metadata::TemplateDependency> {
        let mut merged = base.to_vec();

        for child_dep in child {
            // Check if child overrides a base dependency (by dependency_name if set)
            let should_add = if let Some(child_name) = &child_dep.dependency_name {
                !merged.iter().any(|d| d.dependency_name.as_ref() == Some(child_name))
            } else {
                true
            };

            if should_add {
                merged.push(child_dep.clone());
            }
        }

        merged
    }

    /// Merge hooks - child hooks are prepended to base hooks
    fn merge_hooks(
        base: &Option<super::metadata::HooksConfig>,
        child: &Option<super::metadata::HooksConfig>,
    ) -> Option<super::metadata::HooksConfig> {
        match (base, child) {
            (None, None) => None,
            (Some(b), None) => Some(b.clone()),
            (None, Some(c)) => Some(c.clone()),
            (Some(base_hooks), Some(child_hooks)) => {
                // For each phase, child hooks come first, then base hooks
                let merged = super::metadata::HooksConfig {
                    pre_preview: Self::concat_hooks(&child_hooks.pre_preview, &base_hooks.pre_preview),
                    post_preview: Self::concat_hooks(&child_hooks.post_preview, &base_hooks.post_preview),
                    pre_apply: Self::concat_hooks(&child_hooks.pre_apply, &base_hooks.pre_apply),
                    post_apply: Self::concat_hooks(&child_hooks.post_apply, &base_hooks.post_apply),
                    pre_destroy: Self::concat_hooks(&child_hooks.pre_destroy, &base_hooks.pre_destroy),
                    post_destroy: Self::concat_hooks(&child_hooks.post_destroy, &base_hooks.post_destroy),
                    pre_refresh: Self::concat_hooks(&child_hooks.pre_refresh, &base_hooks.pre_refresh),
                    post_refresh: Self::concat_hooks(&child_hooks.post_refresh, &base_hooks.post_refresh),
                    pre_test: Self::concat_hooks(&child_hooks.pre_test, &base_hooks.pre_test),
                    post_test: Self::concat_hooks(&child_hooks.post_test, &base_hooks.post_test),
                };

                // If all are empty, return None
                if merged.pre_preview.is_empty()
                    && merged.post_preview.is_empty()
                    && merged.pre_apply.is_empty()
                    && merged.post_apply.is_empty()
                    && merged.pre_destroy.is_empty()
                    && merged.post_destroy.is_empty()
                    && merged.pre_refresh.is_empty()
                    && merged.post_refresh.is_empty()
                    && merged.pre_test.is_empty()
                    && merged.post_test.is_empty()
                {
                    return None;
                }

                Some(merged)
            }
        }
    }

    /// Concatenate two hook vectors (first comes before second)
    fn concat_hooks(
        first: &[super::metadata::Hook],
        second: &[super::metadata::Hook],
    ) -> Vec<super::metadata::Hook> {
        let mut result = first.to_vec();
        result.extend(second.iter().cloned());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::metadata::{TemplateDependency, TemplateProjectRef};

    fn make_input(name: &str, description: &str) -> InputDefinition {
        InputDefinition {
            name: name.to_string(),
            description: Some(description.to_string()),
            input_type: None,
            enum_values: None,
            default: None,
            validation: None,
            conditions: vec![],
            secret_manager: None,
        }
    }

    #[test]
    fn test_merge_inputs_child_wins() {
        let base_inputs = vec![
            make_input("shared", "Base description"),
            make_input("base_only", "Only in base"),
        ];

        let child_inputs = vec![
            make_input("shared", "Child description"),
            make_input("child_only", "Only in child"),
        ];

        let merged = TemplateResolver::merge_inputs(&base_inputs, &child_inputs);

        assert_eq!(merged.len(), 3);

        // shared should have child's values
        let shared = merged.iter().find(|i| i.name == "shared").unwrap();
        assert_eq!(shared.description, Some("Child description".to_string()));

        // base_only should be preserved
        assert!(merged.iter().any(|i| i.name == "base_only"));

        // child_only should be added
        assert!(merged.iter().any(|i| i.name == "child_only"));
    }

    #[test]
    fn test_merge_dependencies_concatenate() {
        let base_deps = vec![TemplateDependency {
            dependency_name: Some("vpc".to_string()),
            project: TemplateProjectRef {
                api_version: "pmp.io/v1".to_string(),
                kind: "VPC".to_string(),
                label_selector: HashMap::new(),
                description: None,
                remote_state: None,
            },
        }];

        let child_deps = vec![TemplateDependency {
            dependency_name: Some("database".to_string()),
            project: TemplateProjectRef {
                api_version: "pmp.io/v1".to_string(),
                kind: "Database".to_string(),
                label_selector: HashMap::new(),
                description: None,
                remote_state: None,
            },
        }];

        let merged = TemplateResolver::merge_dependencies(&base_deps, &child_deps);

        assert_eq!(merged.len(), 2);
        assert!(merged
            .iter()
            .any(|d| d.dependency_name == Some("vpc".to_string())));
        assert!(merged
            .iter()
            .any(|d| d.dependency_name == Some("database".to_string())));
    }
}
