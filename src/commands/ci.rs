use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use crate::template::metadata::ProjectReference;
use anyhow::{Context as AnyhowContext, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct CiCommand;

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineType {
    GitHubActions,
    GitLabCI,
    Jenkins,
}

impl PipelineType {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "github" | "github-actions" => Ok(Self::GitHubActions),
            "gitlab" | "gitlab-ci" => Ok(Self::GitLabCI),
            "jenkins" => Ok(Self::Jenkins),
            _ => anyhow::bail!("Unsupported pipeline type: {}", s),
        }
    }
}

#[derive(Debug)]
struct ProjectInfo {
    name: String,
    environment: String,
    path: PathBuf,
    dependencies: Vec<String>, // project:env keys
}

impl CiCommand {
    /// Execute the ci generate command
    pub fn execute_generate(
        ctx: &Context,
        pipeline_type: &str,
        output_file: Option<&str>,
        environment: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("CI/CD Pipeline Generation");

        let pipeline = PipelineType::from_str(pipeline_type)?;

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        ctx.output
            .key_value("Pipeline Type", &format!("{:?}", pipeline));
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Build project info with dependencies
        let project_infos =
            Self::build_project_infos(ctx, &projects, &infrastructure_root, environment)?;

        // Generate pipeline based on type
        let pipeline_content = match pipeline {
            PipelineType::GitHubActions => {
                Self::generate_github_actions(&project_infos, environment)?
            }
            PipelineType::GitLabCI => Self::generate_gitlab_ci(&project_infos, environment)?,
            PipelineType::Jenkins => Self::generate_jenkins(&project_infos, environment)?,
        };

        // Output or save pipeline
        if let Some(file_path) = output_file {
            let output_path = PathBuf::from(file_path);
            ctx.fs.write(&output_path, &pipeline_content)?;
            ctx.output
                .success(&format!("Pipeline written to: {}", file_path));
        } else {
            output::blank();
            ctx.output.info("Generated Pipeline:");
            output::blank();
            ctx.output.info(&pipeline_content);
        }

        Ok(())
    }

    /// Build project information with dependencies
    fn build_project_infos(
        ctx: &Context,
        projects: &[ProjectReference],
        infrastructure_root: &Path,
        filter_environment: Option<&str>,
    ) -> Result<Vec<ProjectInfo>> {
        let mut project_infos = Vec::new();

        for project in projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");
                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) =
                            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                    {
                        let env_name = &resource.metadata.environment_name;

                        // Filter by environment if specified
                        if let Some(filter_env) = filter_environment
                            && env_name != filter_env
                        {
                            continue;
                        }

                        let mut deps = Vec::new();
                        for dep in &resource.spec.dependencies {
                            for dep_env in &dep.project.environments {
                                deps.push(format!("{}:{}", dep.project.name, dep_env));
                            }
                        }

                        project_infos.push(ProjectInfo {
                            name: resource.metadata.name.clone(),
                            environment: env_name.clone(),
                            path: env_path.clone(),
                            dependencies: deps,
                        });
                    }
                }
            }
        }

        Ok(project_infos)
    }

    /// Generate GitHub Actions workflow
    fn generate_github_actions(
        projects: &[ProjectInfo],
        _environment: Option<&str>,
    ) -> Result<String> {
        let mut yaml = String::new();

        yaml.push_str("name: PMP Infrastructure Deployment\n\n");

        yaml.push_str("on:\n");
        yaml.push_str("  push:\n");
        yaml.push_str("    branches:\n");
        yaml.push_str("      - main\n");
        yaml.push_str("  pull_request:\n");
        yaml.push_str("    branches:\n");
        yaml.push_str("      - main\n");
        yaml.push_str("  workflow_dispatch:\n\n");

        yaml.push_str("env:\n");
        yaml.push_str("  TOFU_VERSION: \"1.6.0\"\n\n");

        yaml.push_str("jobs:\n");

        // Group projects by dependency level for parallel execution
        let execution_groups = Self::group_by_dependency_level(projects);

        for (level, group_projects) in execution_groups.iter().enumerate() {
            let stage_name = format!("stage_{}", level);

            yaml.push_str(&format!("  {}:\n", stage_name));
            yaml.push_str("    name: ");
            yaml.push_str(&format!("Deploy Stage {}\n", level));
            yaml.push_str("    runs-on: ubuntu-latest\n");

            if level > 0 {
                yaml.push_str("    needs:\n");
                yaml.push_str(&format!("      - stage_{}\n", level - 1));
            }

            yaml.push_str("    strategy:\n");
            yaml.push_str("      matrix:\n");
            yaml.push_str("        project:\n");

            for proj in group_projects {
                yaml.push_str(&format!("          - name: \"{}\"\n", proj.name));
                yaml.push_str(&format!("            env: \"{}\"\n", proj.environment));
                yaml.push_str(&format!(
                    "            path: \"{}\"\n",
                    proj.path.display().to_string().replace('\\', "/")
                ));
            }

            yaml.push_str("\n    steps:\n");
            yaml.push_str("      - name: Checkout\n");
            yaml.push_str("        uses: actions/checkout@v4\n\n");

            yaml.push_str("      - name: Setup OpenTofu\n");
            yaml.push_str("        uses: opentofu/setup-opentofu@v1\n");
            yaml.push_str("        with:\n");
            yaml.push_str("          tofu_version: ${{ env.TOFU_VERSION }}\n\n");

            yaml.push_str("      - name: Tofu Init\n");
            yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
            yaml.push_str("        run: tofu init\n\n");

            yaml.push_str("      - name: Tofu Validate\n");
            yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
            yaml.push_str("        run: tofu validate\n\n");

            yaml.push_str("      - name: Tofu Plan\n");
            yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
            yaml.push_str("        run: tofu plan -no-color\n\n");

            yaml.push_str("      - name: Tofu Apply\n");
            yaml.push_str(
                "        if: github.ref == 'refs/heads/main' && github.event_name == 'push'\n",
            );
            yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
            yaml.push_str("        run: tofu apply -auto-approve\n\n");
        }

        Ok(yaml)
    }

    /// Generate GitLab CI configuration
    fn generate_gitlab_ci(projects: &[ProjectInfo], _environment: Option<&str>) -> Result<String> {
        let mut yaml = String::new();

        yaml.push_str("# GitLab CI/CD Pipeline for PMP Infrastructure\n\n");

        yaml.push_str("stages:\n");

        // Determine number of stages based on dependency levels
        let execution_groups = Self::group_by_dependency_level(projects);

        for (level, _) in execution_groups.iter().enumerate() {
            yaml.push_str(&format!("  - stage_{}\n", level));
        }

        yaml.push('\n');

        yaml.push_str("variables:\n");
        yaml.push_str("  TOFU_VERSION: \"1.6.0\"\n\n");

        yaml.push_str("default:\n");
        yaml.push_str("  image: alpine:latest\n");
        yaml.push_str("  before_script:\n");
        yaml.push_str("    - apk add --no-cache curl\n");
        yaml.push_str("    - curl -Lo /usr/local/bin/tofu https://github.com/opentofu/opentofu/releases/download/v${TOFU_VERSION}/tofu_${TOFU_VERSION}_linux_amd64.zip\n");
        yaml.push_str("    - chmod +x /usr/local/bin/tofu\n\n");

        // Generate jobs for each stage
        for (level, group_projects) in execution_groups.iter().enumerate() {
            for proj in group_projects {
                let job_name = format!("{}_{}", proj.name.replace('-', "_"), proj.environment);

                yaml.push_str(&format!("{}:\n", job_name));
                yaml.push_str(&format!("  stage: stage_{}\n", level));
                yaml.push_str("  script:\n");
                yaml.push_str(&format!(
                    "    - cd {}\n",
                    proj.path.display().to_string().replace('\\', "/")
                ));
                yaml.push_str("    - tofu init\n");
                yaml.push_str("    - tofu validate\n");
                yaml.push_str("    - tofu plan -no-color\n");
                yaml.push_str("    - |\n");
                yaml.push_str("      if [ \"$CI_COMMIT_BRANCH\" == \"main\" ]; then\n");
                yaml.push_str("        tofu apply -auto-approve\n");
                yaml.push_str("      fi\n");
                yaml.push_str("  rules:\n");
                yaml.push_str("    - if: $CI_PIPELINE_SOURCE == \"merge_request_event\"\n");
                yaml.push_str("    - if: $CI_COMMIT_BRANCH == \"main\"\n\n");
            }
        }

        Ok(yaml)
    }

    /// Generate Jenkins pipeline
    fn generate_jenkins(projects: &[ProjectInfo], _environment: Option<&str>) -> Result<String> {
        let mut groovy = String::new();

        groovy.push_str("// Jenkinsfile for PMP Infrastructure\n\n");

        groovy.push_str("pipeline {\n");
        groovy.push_str("    agent any\n\n");

        groovy.push_str("    environment {\n");
        groovy.push_str("        TOFU_VERSION = '1.6.0'\n");
        groovy.push_str("    }\n\n");

        groovy.push_str("    stages {\n");

        // Group by dependency level
        let execution_groups = Self::group_by_dependency_level(projects);

        for (level, group_projects) in execution_groups.iter().enumerate() {
            groovy.push_str(&format!("        stage('Stage {}') {{\n", level));
            groovy.push_str("            parallel {\n");

            for proj in group_projects {
                groovy.push_str(&format!(
                    "                stage('{}:{}') {{\n",
                    proj.name, proj.environment
                ));
                groovy.push_str("                    steps {\n");
                groovy.push_str(&format!(
                    "                        dir('{}') {{\n",
                    proj.path.display().to_string().replace('\\', "/")
                ));
                groovy.push_str("                            sh 'tofu init'\n");
                groovy.push_str("                            sh 'tofu validate'\n");
                groovy.push_str("                            sh 'tofu plan -no-color'\n");
                groovy.push_str("                            script {\n");
                groovy
                    .push_str("                                if (env.BRANCH_NAME == 'main') {\n");
                groovy.push_str(
                    "                                    sh 'tofu apply -auto-approve'\n",
                );
                groovy.push_str("                                }\n");
                groovy.push_str("                            }\n");
                groovy.push_str("                        }\n");
                groovy.push_str("                    }\n");
                groovy.push_str("                }\n");
            }

            groovy.push_str("            }\n");
            groovy.push_str("        }\n");
        }

        groovy.push_str("    }\n\n");

        groovy.push_str("    post {\n");
        groovy.push_str("        success {\n");
        groovy.push_str("            echo 'Deployment successful!'\n");
        groovy.push_str("        }\n");
        groovy.push_str("        failure {\n");
        groovy.push_str("            echo 'Deployment failed!'\n");
        groovy.push_str("        }\n");
        groovy.push_str("    }\n");
        groovy.push_str("}\n");

        Ok(groovy)
    }

    /// Group projects by dependency level for parallel execution
    fn group_by_dependency_level(projects: &[ProjectInfo]) -> Vec<Vec<&ProjectInfo>> {
        let mut groups: Vec<Vec<&ProjectInfo>> = Vec::new();
        let mut assigned: HashSet<String> = HashSet::new();
        let mut remaining: Vec<&ProjectInfo> = projects.iter().collect();

        while !remaining.is_empty() {
            let mut current_level = Vec::new();

            for project in &remaining {
                let project_key = format!("{}:{}", project.name, project.environment);

                // Check if all dependencies are satisfied
                let deps_satisfied = project
                    .dependencies
                    .iter()
                    .all(|dep| assigned.contains(dep));

                if deps_satisfied {
                    current_level.push(*project);
                    assigned.insert(project_key);
                }
            }

            if current_level.is_empty() {
                // No progress - circular dependency or orphaned projects
                // Add remaining projects to current level to break deadlock
                for project in &remaining {
                    let project_key = format!("{}:{}", project.name, project.environment);
                    if !assigned.contains(&project_key) {
                        current_level.push(*project);
                        assigned.insert(project_key);
                    }
                }
            }

            groups.push(current_level);

            // Update remaining
            remaining.retain(|p| {
                let key = format!("{}:{}", p.name, p.environment);
                !assigned.contains(&key)
            });
        }

        groups
    }
}
