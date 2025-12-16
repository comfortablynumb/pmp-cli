use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::{CostConfig, ProjectReference};
use crate::template::DynamicProjectEnvironmentResource;
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
        static_mode: bool,
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
        ctx.output.key_value(
            "Mode",
            if static_mode {
                "Static (all projects)"
            } else {
                "Dynamic (changed projects)"
            },
        );

        // Get cost configuration
        let cost_config = infrastructure.spec.cost.as_ref();
        let cost_ci_enabled = cost_config
            .and_then(|c| c.ci.as_ref())
            .is_some_and(|ci| ci.enabled);

        if cost_ci_enabled {
            ctx.output.key_value("Cost Estimation", "Enabled");
        }

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

        // Generate pipeline based on type and mode
        let pipeline_content = if static_mode {
            // Static mode: Generate pipeline that runs all projects
            match pipeline {
                PipelineType::GitHubActions => {
                    Self::generate_github_actions_static(&project_infos, environment, cost_config)?
                }
                PipelineType::GitLabCI => {
                    Self::generate_gitlab_ci_static(&project_infos, environment, cost_config)?
                }
                PipelineType::Jenkins => {
                    Self::generate_jenkins_static(&project_infos, environment, cost_config)?
                }
            }
        } else {
            // Dynamic mode: Generate pipeline with change detection
            match pipeline {
                PipelineType::GitHubActions => {
                    Self::generate_github_actions_dynamic(&project_infos, environment, cost_config)?
                }
                PipelineType::GitLabCI => {
                    Self::generate_gitlab_ci_dynamic(&project_infos, environment, cost_config)?
                }
                PipelineType::Jenkins => {
                    // Jenkins doesn't support dynamic mode yet, fall back to static
                    ctx.output.warning(
                        "Jenkins does not support dynamic mode yet. Generating static pipeline.",
                    );
                    Self::generate_jenkins_static(&project_infos, environment, cost_config)?
                }
            }
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

    /// Generate static GitHub Actions workflow (runs all projects)
    fn generate_github_actions_static(
        projects: &[ProjectInfo],
        _environment: Option<&str>,
        cost_config: Option<&CostConfig>,
    ) -> Result<String> {
        let mut yaml = String::new();

        // Get cost CI settings
        let cost_ci = cost_config.and_then(|c| c.ci.as_ref());
        let cost_enabled = cost_ci.is_some_and(|ci| ci.enabled);
        let comment_on_pr = cost_ci.is_some_and(|ci| ci.comment_on_pr);
        let fail_on_threshold = cost_ci.is_some_and(|ci| ci.fail_on_threshold);

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
        yaml.push_str("  TOFU_VERSION: \"1.6.0\"\n");

        if cost_enabled {
            yaml.push_str("  INFRACOST_API_KEY: ${{ secrets.INFRACOST_API_KEY }}\n");
        }

        yaml.push_str("\n");
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

            yaml.push_str("      - name: Install PMP\n");
            yaml.push_str("        run: |\n");
            yaml.push_str("          curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash\n");
            yaml.push_str("          echo \"$HOME/.pmp/bin\" >> $GITHUB_PATH\n\n");

            // Add Infracost setup if cost estimation is enabled
            if cost_enabled {
                yaml.push_str("      - name: Setup Infracost\n");
                yaml.push_str("        uses: infracost/actions/setup@v3\n");
                yaml.push_str("        with:\n");
                yaml.push_str("          api-key: ${{ env.INFRACOST_API_KEY }}\n\n");
            }

            yaml.push_str("      - name: Preview (Plan)\n");
            yaml.push_str("        if: github.event_name == 'pull_request'\n");
            yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");

            if cost_enabled && fail_on_threshold {
                yaml.push_str("        run: pmp project preview --cost\n\n");
            } else {
                yaml.push_str("        run: pmp project preview\n\n");
            }

            // Add cost estimation step for PRs
            if cost_enabled {
                yaml.push_str("      - name: Cost Estimation\n");
                yaml.push_str("        if: github.event_name == 'pull_request'\n");
                yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
                yaml.push_str("        run: |\n");
                yaml.push_str("          pmp cost diff\n");

                if fail_on_threshold {
                    yaml.push_str("        continue-on-error: false\n\n");
                } else {
                    yaml.push_str("        continue-on-error: true\n\n");
                }

                // Add PR comment step if enabled
                if comment_on_pr {
                    yaml.push_str("      - name: Generate Cost Report\n");
                    yaml.push_str("        if: github.event_name == 'pull_request'\n");
                    yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
                    yaml.push_str("        run: |\n");
                    yaml.push_str("          infracost breakdown --path . --format json > /tmp/infracost-${{ matrix.project.name }}.json\n");
                    yaml.push_str("        continue-on-error: true\n\n");

                    yaml.push_str("      - name: Post Cost Comment\n");
                    yaml.push_str("        if: github.event_name == 'pull_request'\n");
                    yaml.push_str("        uses: infracost/actions/comment@v1\n");
                    yaml.push_str("        with:\n");
                    yaml.push_str("          path: /tmp/infracost-${{ matrix.project.name }}.json\n");
                    yaml.push_str("          behavior: update\n\n");
                }
            }

            yaml.push_str("      - name: Apply\n");
            yaml.push_str(
                "        if: github.ref == 'refs/heads/main' && github.event_name == 'push'\n",
            );
            yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");

            if cost_enabled && fail_on_threshold {
                yaml.push_str("        run: pmp project apply --cost\n\n");
            } else {
                yaml.push_str("        run: pmp project apply\n\n");
            }
        }

        Ok(yaml)
    }

    /// Generate dynamic GitHub Actions workflow (runs only changed projects)
    fn generate_github_actions_dynamic(
        _projects: &[ProjectInfo],
        _environment: Option<&str>,
        cost_config: Option<&CostConfig>,
    ) -> Result<String> {
        let mut yaml = String::new();

        // Get cost CI settings
        let cost_ci = cost_config.and_then(|c| c.ci.as_ref());
        let cost_enabled = cost_ci.is_some_and(|ci| ci.enabled);
        let comment_on_pr = cost_ci.is_some_and(|ci| ci.comment_on_pr);
        let fail_on_threshold = cost_ci.is_some_and(|ci| ci.fail_on_threshold);

        yaml.push_str("name: PMP Infrastructure Deployment\n\n");

        yaml.push_str("on:\n");
        yaml.push_str("  push:\n");
        yaml.push_str("    branches:\n");
        yaml.push_str("      - main\n");
        yaml.push_str("    tags:\n");
        yaml.push_str("      - '*'\n");
        yaml.push_str("  pull_request:\n");
        yaml.push_str("    branches:\n");
        yaml.push_str("      - main\n");
        yaml.push_str("  workflow_dispatch:\n\n");

        yaml.push_str("env:\n");
        yaml.push_str("  TOFU_VERSION: \"1.6.0\"\n");

        if cost_enabled {
            yaml.push_str("  INFRACOST_API_KEY: ${{ secrets.INFRACOST_API_KEY }}\n");
        }

        yaml.push_str("\n");
        yaml.push_str("jobs:\n");

        // Detect changes job
        yaml.push_str("  detect-changes:\n");
        yaml.push_str("    name: Detect Changed Projects\n");
        yaml.push_str("    runs-on: ubuntu-latest\n");
        yaml.push_str("    outputs:\n");
        yaml.push_str("      projects: ${{ steps.detect.outputs.projects }}\n");
        yaml.push_str("      has_changes: ${{ steps.detect.outputs.has_changes }}\n");
        yaml.push_str("    steps:\n");
        yaml.push_str("      - name: Checkout\n");
        yaml.push_str("        uses: actions/checkout@v4\n");
        yaml.push_str("        with:\n");
        yaml.push_str("          fetch-depth: 0  # Need full history for git diff\n\n");

        yaml.push_str("      - name: Install PMP\n");
        yaml.push_str("        run: |\n");
        yaml.push_str("          curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash\n");
        yaml.push_str("          echo \"$HOME/.pmp/bin\" >> $GITHUB_PATH\n\n");

        yaml.push_str("      - name: Detect changed projects\n");
        yaml.push_str("        id: detect\n");
        yaml.push_str("        run: |\n");
        yaml.push_str("          # Determine base ref based on event type\n");
        yaml.push_str("          if [ \"${{ github.event_name }}\" = \"pull_request\" ]; then\n");
        yaml.push_str(
            "            BASE_REF=\"origin/${{ github.event.pull_request.base.ref }}\"\n",
        );
        yaml.push_str("          else\n");
        yaml.push_str("            BASE_REF=\"origin/main\"\n");
        yaml.push_str("          fi\n");
        yaml.push_str("          \n");
        yaml.push_str("          HEAD_REF=\"${{ github.sha }}\"\n");
        yaml.push_str("          \n");
        yaml.push_str("          # Run PMP detect-changes command\n");
        yaml.push_str("          PROJECTS=$(pmp ci detect-changes --base \"$BASE_REF\" --head \"$HEAD_REF\" --output-format json 2>&1) || EXIT_CODE=$?\n");
        yaml.push_str("          \n");
        yaml.push_str("          # Check exit code\n");
        yaml.push_str("          if [ \"${EXIT_CODE:-0}\" -eq 2 ]; then\n");
        yaml.push_str(
            "            echo \"Infrastructure configuration changed - skipping project CI\"\n",
        );
        yaml.push_str("            echo \"has_changes=false\" >> $GITHUB_OUTPUT\n");
        yaml.push_str("            echo \"projects=[]\" >> $GITHUB_OUTPUT\n");
        yaml.push_str("            exit 0\n");
        yaml.push_str("          fi\n");
        yaml.push_str("          \n");
        yaml.push_str("          # Output results\n");
        yaml.push_str("          echo \"projects=$PROJECTS\" >> $GITHUB_OUTPUT\n");
        yaml.push_str("          if [ \"$PROJECTS\" = \"[]\" ]; then\n");
        yaml.push_str("            echo \"has_changes=false\" >> $GITHUB_OUTPUT\n");
        yaml.push_str("          else\n");
        yaml.push_str("            echo \"has_changes=true\" >> $GITHUB_OUTPUT\n");
        yaml.push_str("          fi\n\n");

        // Preview job (on PR)
        yaml.push_str("  preview:\n");
        yaml.push_str("    name: Preview ${{ matrix.project.name }} (${{ matrix.project.env }})\n");
        yaml.push_str("    needs: detect-changes\n");
        yaml.push_str("    if: github.event_name == 'pull_request' && needs.detect-changes.outputs.has_changes == 'true'\n");
        yaml.push_str("    runs-on: ubuntu-latest\n");
        yaml.push_str("    strategy:\n");
        yaml.push_str("      fail-fast: false\n");
        yaml.push_str("      matrix:\n");
        yaml.push_str("        project: ${{ fromJSON(needs.detect-changes.outputs.projects) }}\n");
        yaml.push_str("    steps:\n");
        yaml.push_str("      - name: Checkout\n");
        yaml.push_str("        uses: actions/checkout@v4\n\n");

        yaml.push_str("      - name: Setup OpenTofu\n");
        yaml.push_str("        uses: opentofu/setup-opentofu@v1\n");
        yaml.push_str("        with:\n");
        yaml.push_str("          tofu_version: ${{ env.TOFU_VERSION }}\n\n");

        yaml.push_str("      - name: Install PMP\n");
        yaml.push_str("        run: |\n");
        yaml.push_str("          curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash\n");
        yaml.push_str("          echo \"$HOME/.pmp/bin\" >> $GITHUB_PATH\n\n");

        // Add Infracost setup if cost estimation is enabled
        if cost_enabled {
            yaml.push_str("      - name: Setup Infracost\n");
            yaml.push_str("        uses: infracost/actions/setup@v3\n");
            yaml.push_str("        with:\n");
            yaml.push_str("          api-key: ${{ env.INFRACOST_API_KEY }}\n\n");
        }

        yaml.push_str("      - name: Preview changes\n");
        yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");

        if cost_enabled && fail_on_threshold {
            yaml.push_str("        run: pmp project preview --cost\n\n");
        } else {
            yaml.push_str("        run: pmp project preview\n\n");
        }

        // Add cost estimation step for PRs
        if cost_enabled {
            yaml.push_str("      - name: Cost Estimation\n");
            yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
            yaml.push_str("        run: |\n");
            yaml.push_str("          pmp cost diff\n");

            if fail_on_threshold {
                yaml.push_str("        continue-on-error: false\n\n");
            } else {
                yaml.push_str("        continue-on-error: true\n\n");
            }

            // Add PR comment step if enabled
            if comment_on_pr {
                yaml.push_str("      - name: Generate Cost Report\n");
                yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");
                yaml.push_str("        run: |\n");
                yaml.push_str("          infracost breakdown --path . --format json > /tmp/infracost-${{ matrix.project.name }}.json\n");
                yaml.push_str("        continue-on-error: true\n\n");

                yaml.push_str("      - name: Post Cost Comment\n");
                yaml.push_str("        uses: infracost/actions/comment@v1\n");
                yaml.push_str("        with:\n");
                yaml.push_str("          path: /tmp/infracost-${{ matrix.project.name }}.json\n");
                yaml.push_str("          behavior: update\n\n");
            }
        }

        // Apply job (on push to main or tags)
        yaml.push_str("  apply:\n");
        yaml.push_str("    name: Apply ${{ matrix.project.name }} (${{ matrix.project.env }})\n");
        yaml.push_str("    needs: detect-changes\n");
        yaml.push_str("    if: (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/')) && github.event_name == 'push' && needs.detect-changes.outputs.has_changes == 'true'\n");
        yaml.push_str("    runs-on: ubuntu-latest\n");
        yaml.push_str("    strategy:\n");
        yaml.push_str("      fail-fast: false\n");
        yaml.push_str("      matrix:\n");
        yaml.push_str("        project: ${{ fromJSON(needs.detect-changes.outputs.projects) }}\n");
        yaml.push_str("    steps:\n");
        yaml.push_str("      - name: Checkout\n");
        yaml.push_str("        uses: actions/checkout@v4\n\n");

        yaml.push_str("      - name: Setup OpenTofu\n");
        yaml.push_str("        uses: opentofu/setup-opentofu@v1\n");
        yaml.push_str("        with:\n");
        yaml.push_str("          tofu_version: ${{ env.TOFU_VERSION }}\n\n");

        yaml.push_str("      - name: Install PMP\n");
        yaml.push_str("        run: |\n");
        yaml.push_str("          curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash\n");
        yaml.push_str("          echo \"$HOME/.pmp/bin\" >> $GITHUB_PATH\n\n");

        // Add Infracost setup if cost estimation is enabled
        if cost_enabled {
            yaml.push_str("      - name: Setup Infracost\n");
            yaml.push_str("        uses: infracost/actions/setup@v3\n");
            yaml.push_str("        with:\n");
            yaml.push_str("          api-key: ${{ env.INFRACOST_API_KEY }}\n\n");
        }

        yaml.push_str("      - name: Apply changes\n");
        yaml.push_str("        working-directory: ${{ matrix.project.path }}\n");

        if cost_enabled && fail_on_threshold {
            yaml.push_str("        run: pmp project apply --cost\n\n");
        } else {
            yaml.push_str("        run: pmp project apply\n\n");
        }

        Ok(yaml)
    }

    /// Generate static GitLab CI configuration (runs all projects)
    fn generate_gitlab_ci_static(
        projects: &[ProjectInfo],
        _environment: Option<&str>,
        cost_config: Option<&CostConfig>,
    ) -> Result<String> {
        let mut yaml = String::new();

        // Get cost CI settings
        let cost_ci = cost_config.and_then(|c| c.ci.as_ref());
        let cost_enabled = cost_ci.is_some_and(|ci| ci.enabled);
        let fail_on_threshold = cost_ci.is_some_and(|ci| ci.fail_on_threshold);

        yaml.push_str("# GitLab CI/CD Pipeline for PMP Infrastructure\n\n");

        yaml.push_str("stages:\n");

        // Determine number of stages based on dependency levels
        let execution_groups = Self::group_by_dependency_level(projects);

        for (level, _) in execution_groups.iter().enumerate() {
            yaml.push_str(&format!("  - stage_{}\n", level));
        }

        yaml.push('\n');

        yaml.push_str("variables:\n");
        yaml.push_str("  TOFU_VERSION: \"1.6.0\"\n");

        if cost_enabled {
            yaml.push_str("  INFRACOST_API_KEY: $INFRACOST_API_KEY\n");
        }

        yaml.push('\n');

        yaml.push_str("default:\n");
        yaml.push_str("  image: alpine:latest\n");
        yaml.push_str("  before_script:\n");
        yaml.push_str("    - apk add --no-cache curl\n");
        yaml.push_str("    - curl -Lo /usr/local/bin/tofu https://github.com/opentofu/opentofu/releases/download/v${TOFU_VERSION}/tofu_${TOFU_VERSION}_linux_amd64.zip\n");
        yaml.push_str("    - chmod +x /usr/local/bin/tofu\n");
        yaml.push_str("    - curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash\n");
        yaml.push_str("    - export PATH=\"$HOME/.pmp/bin:$PATH\"\n");

        if cost_enabled {
            yaml.push_str("    - |\n");
            yaml.push_str("      # Install Infracost\n");
            yaml.push_str("      curl -fsSL https://raw.githubusercontent.com/infracost/infracost/master/scripts/install.sh | sh\n");
        }

        yaml.push('\n');

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
                yaml.push_str("    - |\n");
                yaml.push_str("      # Run preview on MR, apply on main branch\n");
                yaml.push_str(
                    "      if [ \"$CI_PIPELINE_SOURCE\" == \"merge_request_event\" ]; then\n",
                );

                if cost_enabled && fail_on_threshold {
                    yaml.push_str("        pmp project preview --cost\n");
                    yaml.push_str("        pmp cost diff\n");
                } else if cost_enabled {
                    yaml.push_str("        pmp project preview\n");
                    yaml.push_str("        pmp cost diff || true\n");
                } else {
                    yaml.push_str("        pmp project preview\n");
                }

                yaml.push_str("      elif [ \"$CI_COMMIT_BRANCH\" == \"main\" ]; then\n");

                if cost_enabled && fail_on_threshold {
                    yaml.push_str("        pmp project apply --cost\n");
                } else {
                    yaml.push_str("        pmp project apply\n");
                }

                yaml.push_str("      fi\n");
                yaml.push_str("  rules:\n");
                yaml.push_str("    - if: $CI_PIPELINE_SOURCE == \"merge_request_event\"\n");
                yaml.push_str("    - if: $CI_COMMIT_BRANCH == \"main\"\n\n");
            }
        }

        Ok(yaml)
    }

    /// Generate dynamic GitLab CI configuration (runs only changed projects)
    fn generate_gitlab_ci_dynamic(
        _projects: &[ProjectInfo],
        _environment: Option<&str>,
        cost_config: Option<&CostConfig>,
    ) -> Result<String> {
        let mut yaml = String::new();

        // Get cost CI settings
        let cost_ci = cost_config.and_then(|c| c.ci.as_ref());
        let cost_enabled = cost_ci.is_some_and(|ci| ci.enabled);
        let fail_on_threshold = cost_ci.is_some_and(|ci| ci.fail_on_threshold);

        yaml.push_str(
            "# GitLab CI/CD Pipeline for PMP Infrastructure (Dynamic - Change Detection)\n\n",
        );

        yaml.push_str("stages:\n");
        yaml.push_str("  - detect\n");
        yaml.push_str("  - preview\n");
        yaml.push_str("  - apply\n\n");

        yaml.push_str("variables:\n");
        yaml.push_str("  TOFU_VERSION: \"1.6.0\"\n");

        if cost_enabled {
            yaml.push_str("  INFRACOST_API_KEY: $INFRACOST_API_KEY\n");
        }

        yaml.push('\n');

        yaml.push_str("default:\n");
        yaml.push_str("  image: alpine:latest\n");
        yaml.push_str("  before_script:\n");
        yaml.push_str("    - apk add --no-cache curl git jq\n");
        yaml.push_str("    - |\n");
        yaml.push_str("      # Download and install OpenTofu\n");
        yaml.push_str("      curl -Lo /tmp/tofu.tar.gz https://github.com/opentofu/opentofu/releases/download/v${TOFU_VERSION}/tofu_${TOFU_VERSION}_linux_amd64.tar.gz\n");
        yaml.push_str("      tar -xzf /tmp/tofu.tar.gz -C /usr/local/bin\n");
        yaml.push_str("      chmod +x /usr/local/bin/tofu\n");
        yaml.push_str("    - curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash\n");
        yaml.push_str("    - export PATH=\"$HOME/.pmp/bin:$PATH\"\n");

        if cost_enabled {
            yaml.push_str("    - |\n");
            yaml.push_str("      # Install Infracost\n");
            yaml.push_str("      curl -fsSL https://raw.githubusercontent.com/infracost/infracost/master/scripts/install.sh | sh\n");
        }

        yaml.push('\n');

        // Detect changes job
        yaml.push_str("detect-changes:\n");
        yaml.push_str("  stage: detect\n");
        yaml.push_str("  before_script:\n");
        yaml.push_str("    - apk add --no-cache git curl\n");
        yaml.push_str("    - curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash\n");
        yaml.push_str("    - export PATH=\"$HOME/.pmp/bin:$PATH\"\n");
        yaml.push_str("  script:\n");
        yaml.push_str("    - |\n");
        yaml.push_str("      # Determine base ref\n");
        yaml.push_str("      if [ -n \"$CI_MERGE_REQUEST_TARGET_BRANCH_NAME\" ]; then\n");
        yaml.push_str("        BASE_REF=\"origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME\"\n");
        yaml.push_str("      else\n");
        yaml.push_str("        BASE_REF=\"origin/main\"\n");
        yaml.push_str("      fi\n");
        yaml.push_str("      \n");
        yaml.push_str("      HEAD_REF=\"$CI_COMMIT_SHA\"\n");
        yaml.push_str("      \n");
        yaml.push_str("      # Run PMP detect-changes\n");
        yaml.push_str("      PROJECTS=$(pmp ci detect-changes --base \"$BASE_REF\" --head \"$HEAD_REF\" --output-format json 2>&1) || EXIT_CODE=$?\n");
        yaml.push_str("      \n");
        yaml.push_str("      if [ \"${EXIT_CODE:-0}\" -eq 2 ]; then\n");
        yaml.push_str("        echo \"Infrastructure changed - skipping project CI\"\n");
        yaml.push_str("        echo \"CHANGED_PROJECTS=[]\" >> variables.env\n");
        yaml.push_str("        echo \"HAS_CHANGES=false\" >> variables.env\n");
        yaml.push_str("        exit 0\n");
        yaml.push_str("      fi\n");
        yaml.push_str("      \n");
        yaml.push_str("      echo \"CHANGED_PROJECTS=$PROJECTS\" >> variables.env\n");
        yaml.push_str("      if [ \"$PROJECTS\" = \"[]\" ]; then\n");
        yaml.push_str("        echo \"HAS_CHANGES=false\" >> variables.env\n");
        yaml.push_str("      else\n");
        yaml.push_str("        echo \"HAS_CHANGES=true\" >> variables.env\n");
        yaml.push_str("      fi\n");
        yaml.push_str("  artifacts:\n");
        yaml.push_str("    reports:\n");
        yaml.push_str("      dotenv: variables.env\n\n");

        // Preview job (for MRs)
        yaml.push_str("preview-projects:\n");
        yaml.push_str("  stage: preview\n");
        yaml.push_str("  needs:\n");
        yaml.push_str("    - job: detect-changes\n");
        yaml.push_str("      artifacts: true\n");
        yaml.push_str("  rules:\n");
        yaml.push_str("    - if: $CI_PIPELINE_SOURCE == \"merge_request_event\" && $HAS_CHANGES == \"true\"\n");
        yaml.push_str("  script:\n");
        yaml.push_str("    - |\n");
        yaml.push_str("      # Parse CHANGED_PROJECTS JSON and run pmp project preview for each\n");
        yaml.push_str("      echo \"$CHANGED_PROJECTS\" | jq -r '.[] | \"\\(.path)\"' | while read -r project_path; do\n");
        yaml.push_str("        echo \"Previewing project: $project_path\"\n");
        yaml.push_str("        cd \"$project_path\"\n");

        if cost_enabled && fail_on_threshold {
            yaml.push_str("        pmp project preview --cost\n");
            yaml.push_str("        pmp cost diff\n");
        } else if cost_enabled {
            yaml.push_str("        pmp project preview\n");
            yaml.push_str("        pmp cost diff || true\n");
        } else {
            yaml.push_str("        pmp project preview\n");
        }

        yaml.push_str("        cd -\n");
        yaml.push_str("      done\n\n");

        // Apply job (on push to main)
        yaml.push_str("apply-projects:\n");
        yaml.push_str("  stage: apply\n");
        yaml.push_str("  needs:\n");
        yaml.push_str("    - job: detect-changes\n");
        yaml.push_str("      artifacts: true\n");
        yaml.push_str("  rules:\n");
        yaml.push_str("    - if: $CI_COMMIT_BRANCH == \"main\" && $CI_PIPELINE_SOURCE == \"push\" && $HAS_CHANGES == \"true\"\n");
        yaml.push_str("    - if: $CI_COMMIT_TAG && $HAS_CHANGES == \"true\"\n");
        yaml.push_str("  script:\n");
        yaml.push_str("    - |\n");
        yaml.push_str("      # Parse CHANGED_PROJECTS JSON and run pmp project apply for each\n");
        yaml.push_str("      echo \"$CHANGED_PROJECTS\" | jq -r '.[] | \"\\(.path)\"' | while read -r project_path; do\n");
        yaml.push_str("        echo \"Applying project: $project_path\"\n");
        yaml.push_str("        cd \"$project_path\"\n");

        if cost_enabled && fail_on_threshold {
            yaml.push_str("        pmp project apply --cost\n");
        } else {
            yaml.push_str("        pmp project apply\n");
        }

        yaml.push_str("        cd -\n");
        yaml.push_str("      done\n\n");

        yaml.push_str(
            "# NOTE: This implementation uses jq to parse the JSON array of changed projects\n",
        );
        yaml.push_str("# and runs pmp project preview/apply for each project in sequence.\n");
        yaml.push_str("# For parallel execution, consider using GitLab dynamic child pipelines.\n");

        Ok(yaml)
    }

    /// Generate static Jenkins pipeline (runs all projects)
    fn generate_jenkins_static(
        projects: &[ProjectInfo],
        _environment: Option<&str>,
        cost_config: Option<&CostConfig>,
    ) -> Result<String> {
        let mut groovy = String::new();

        // Get cost CI settings
        let cost_ci = cost_config.and_then(|c| c.ci.as_ref());
        let cost_enabled = cost_ci.is_some_and(|ci| ci.enabled);
        let fail_on_threshold = cost_ci.is_some_and(|ci| ci.fail_on_threshold);

        groovy.push_str("// Jenkinsfile for PMP Infrastructure\n\n");

        groovy.push_str("pipeline {\n");
        groovy.push_str("    agent any\n\n");

        groovy.push_str("    environment {\n");
        groovy.push_str("        TOFU_VERSION = '1.6.0'\n");

        if cost_enabled {
            groovy.push_str("        INFRACOST_API_KEY = credentials('infracost-api-key')\n");
        }

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
                groovy.push_str("                            script {\n");
                groovy.push_str(
                    "                                // Run preview on PR, apply on main branch\n",
                );
                groovy.push_str("                                if (env.CHANGE_ID) {\n");
                groovy.push_str("                                    // Pull request\n");

                if cost_enabled && fail_on_threshold {
                    groovy.push_str("                                    sh 'pmp project preview --cost'\n");
                    groovy.push_str("                                    sh 'pmp cost diff'\n");
                } else if cost_enabled {
                    groovy.push_str("                                    sh 'pmp project preview'\n");
                    groovy.push_str("                                    sh 'pmp cost diff || true'\n");
                } else {
                    groovy.push_str("                                    sh 'pmp project preview'\n");
                }

                groovy.push_str(
                    "                                } else if (env.BRANCH_NAME == 'main') {\n",
                );
                groovy.push_str("                                    // Main branch\n");

                if cost_enabled && fail_on_threshold {
                    groovy.push_str("                                    sh 'pmp project apply --cost'\n");
                } else {
                    groovy.push_str("                                    sh 'pmp project apply'\n");
                }

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
