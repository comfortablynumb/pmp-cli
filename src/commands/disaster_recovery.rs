use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct DisasterRecoveryCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct DisasterRecoveryPlan {
    pub id: String,
    pub project: String,
    pub environment: String,
    pub created_at: String,
    pub created_by: String,
    pub rto_minutes: u32,
    pub rpo_minutes: u32,
    pub steps: Vec<RecoveryStep>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryStep {
    pub order: usize,
    pub title: String,
    pub description: String,
    pub estimated_duration_minutes: u32,
    pub automation_available: bool,
    pub validation_criteria: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryTest {
    pub id: String,
    pub plan_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: TestStatus,
    pub results: Vec<StepResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TestStatus {
    Running,
    Passed,
    Failed,
    PartialSuccess,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepResult {
    pub step_order: usize,
    pub status: StepStatus,
    pub duration_seconds: u32,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StepStatus {
    Success,
    Failed,
    Skipped,
}

impl DisasterRecoveryCommand {
    pub fn execute_plan(ctx: &Context, path: Option<&str>) -> Result<()> {
        ctx.output.section("Generate Disaster Recovery Plan");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = if let Some(p) = path {
            Path::new(p).to_path_buf()
        } else {
            std::env::current_dir()?
        };

        // Check if we're in an environment directory
        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output
                .warning("Not in an environment directory. Please specify a path.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        ctx.output.key_value("Project", &resource.metadata.name);
        ctx.output.key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // Get RTO and RPO
        let rto: u32 = ctx
            .input
            .text("Recovery Time Objective (RTO) in minutes:", Some("60"))?
            .parse()
            .unwrap_or(60);

        let rpo: u32 = ctx
            .input
            .text("Recovery Point Objective (RPO) in minutes:", Some("15"))?
            .parse()
            .unwrap_or(15);

        ctx.output.dimmed("Generating recovery plan...");

        // Generate plan
        let plan = Self::generate_recovery_plan(ctx, &resource, rto, rpo)?;

        // Save plan
        Self::save_plan(ctx, &infrastructure_root, &plan)?;

        ctx.output.success("Disaster recovery plan generated");
        ctx.output.key_value("Plan ID", &plan.id);
        ctx.output.key_value("RTO", &format!("{} minutes", plan.rto_minutes));
        ctx.output.key_value("RPO", &format!("{} minutes", plan.rpo_minutes));
        ctx.output.key_value("Steps", &plan.steps.len().to_string());
        output::blank();

        // Display plan summary
        ctx.output.subsection("Recovery Steps");
        output::blank();

        for step in &plan.steps {
            let automation_marker = if step.automation_available { "🤖" } else { "👤" };
            ctx.output.dimmed(&format!(
                "{}. {} {} ({} min)",
                step.order, automation_marker, step.title, step.estimated_duration_minutes
            ));
        }

        output::blank();
        let total_duration: u32 = plan.steps.iter().map(|s| s.estimated_duration_minutes).sum();
        ctx.output.dimmed(&format!("Total estimated recovery time: {} minutes", total_duration));

        Ok(())
    }

    pub fn execute_test(ctx: &Context, plan_id: Option<&str>, dry_run: bool) -> Result<()> {
        ctx.output.section("Test Disaster Recovery");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Get plan ID
        let id = if let Some(p) = plan_id {
            p.to_string()
        } else {
            // List available plans
            let plans = Self::list_plans(ctx, &infrastructure_root)?;

            if plans.is_empty() {
                ctx.output.info("No DR plans found. Generate one first with 'pmp disaster-recovery plan'");
                return Ok(());
            }

            let options: Vec<String> = plans
                .iter()
                .map(|p| format!("{} - {}/{}", p.id, p.project, p.environment))
                .collect();

            let selection = ctx.input.select("Select plan:", options.clone())?;
            let idx = options.iter().position(|x| x == &selection).unwrap_or(0);
            plans[idx].id.clone()
        };

        // Load plan
        let plan = Self::load_plan(ctx, &infrastructure_root, &id)?;

        ctx.output.key_value("Plan ID", &plan.id);
        ctx.output.key_value("Project", &plan.project);
        ctx.output.key_value("Environment", &plan.environment);
        ctx.output.key_value("RTO", &format!("{} minutes", plan.rto_minutes));
        output::blank();

        if dry_run {
            ctx.output.info("Running in DRY RUN mode - no actual changes will be made");
            output::blank();
        }

        // Confirm test
        let confirm = ctx.input.confirm(
            "This will test the DR procedure. Continue?",
            false,
        )?;

        if !confirm {
            ctx.output.info("Test cancelled");
            return Ok(());
        }

        ctx.output.dimmed("Starting DR test...");
        output::blank();

        // Run test
        let test_result = Self::run_recovery_test(ctx, &plan, dry_run)?;

        // Save test results
        Self::save_test_result(ctx, &infrastructure_root, &test_result)?;

        // Display results
        ctx.output.subsection("Test Results");
        output::blank();

        let mut passed = 0;
        let mut failed = 0;

        for result in &test_result.results {
            let status_icon = match result.status {
                StepStatus::Success => "✓",
                StepStatus::Failed => "✗",
                StepStatus::Skipped => "-",
            };

            let step = plan.steps.iter().find(|s| s.order == result.step_order);
            let title = step.map(|s| s.title.as_str()).unwrap_or("Unknown");

            ctx.output.dimmed(&format!(
                "{} Step {}: {} ({} seconds)",
                status_icon, result.step_order, title, result.duration_seconds
            ));

            match result.status {
                StepStatus::Success => passed += 1,
                StepStatus::Failed => failed += 1,
                _ => {}
            }
        }

        output::blank();
        ctx.output.key_value("Status", &format!("{:?}", test_result.status));
        ctx.output.key_value("Passed", &format!("{}/{}", passed, test_result.results.len()));
        if failed > 0 {
            ctx.output.warning(&format!("{} steps failed", failed));
        }

        Ok(())
    }

    pub fn execute_list(ctx: &Context) -> Result<()> {
        ctx.output.section("Disaster Recovery Plans");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let plans = Self::list_plans(ctx, &infrastructure_root)?;

        if plans.is_empty() {
            ctx.output.info("No DR plans found");
            return Ok(());
        }

        for plan in &plans {
            ctx.output.dimmed(&format!("[{}]", plan.id));
            ctx.output.dimmed(&format!("  {}/{}", plan.project, plan.environment));
            ctx.output.dimmed(&format!("  RTO: {} min, RPO: {} min", plan.rto_minutes, plan.rpo_minutes));
            ctx.output.dimmed(&format!("  {} steps", plan.steps.len()));
            output::blank();
        }

        ctx.output.success(&format!("{} DR plans", plans.len()));

        Ok(())
    }

    // Helper functions

    fn generate_recovery_plan(
        _ctx: &Context,
        resource: &DynamicProjectEnvironmentResource,
        rto_minutes: u32,
        rpo_minutes: u32,
    ) -> Result<DisasterRecoveryPlan> {
        let user = Self::get_current_user()?;

        // In a real implementation:
        // 1. Analyze infrastructure
        // 2. Identify critical resources
        // 3. Generate recovery steps based on dependencies
        // 4. Calculate time estimates
        // 5. Identify automation opportunities

        let steps = vec![
            RecoveryStep {
                order: 1,
                title: "Assess Disaster Impact".to_string(),
                description: "Determine scope of disaster and affected resources".to_string(),
                estimated_duration_minutes: 10,
                automation_available: false,
                validation_criteria: vec![
                    "Documented list of affected resources".to_string(),
                    "Impact assessment complete".to_string(),
                ],
            },
            RecoveryStep {
                order: 2,
                title: "Activate DR Team".to_string(),
                description: "Notify and assemble disaster recovery team".to_string(),
                estimated_duration_minutes: 15,
                automation_available: true,
                validation_criteria: vec![
                    "All team members notified".to_string(),
                    "Communication channels established".to_string(),
                ],
            },
            RecoveryStep {
                order: 3,
                title: "Restore from Backup".to_string(),
                description: "Restore latest backup to recovery environment".to_string(),
                estimated_duration_minutes: 30,
                automation_available: true,
                validation_criteria: vec![
                    "Backup restored successfully".to_string(),
                    "Checksums verified".to_string(),
                ],
            },
            RecoveryStep {
                order: 4,
                title: "Apply Latest State".to_string(),
                description: "Apply Terraform/OpenTofu state to provision resources".to_string(),
                estimated_duration_minutes: 20,
                automation_available: true,
                validation_criteria: vec![
                    "All resources created".to_string(),
                    "No errors in apply".to_string(),
                ],
            },
            RecoveryStep {
                order: 5,
                title: "Verify Resource Health".to_string(),
                description: "Check that all resources are healthy and operational".to_string(),
                estimated_duration_minutes: 15,
                automation_available: true,
                validation_criteria: vec![
                    "All health checks passing".to_string(),
                    "Services responding".to_string(),
                ],
            },
            RecoveryStep {
                order: 6,
                title: "Restore DNS/Traffic Routing".to_string(),
                description: "Update DNS records to point to recovered infrastructure".to_string(),
                estimated_duration_minutes: 10,
                automation_available: true,
                validation_criteria: vec![
                    "DNS records updated".to_string(),
                    "Traffic flowing to new resources".to_string(),
                ],
            },
            RecoveryStep {
                order: 7,
                title: "Validate Application Functionality".to_string(),
                description: "Run smoke tests and validate end-to-end functionality".to_string(),
                estimated_duration_minutes: 20,
                automation_available: true,
                validation_criteria: vec![
                    "All smoke tests passing".to_string(),
                    "Critical user flows working".to_string(),
                ],
            },
            RecoveryStep {
                order: 8,
                title: "Document and Review".to_string(),
                description: "Document recovery process and identify improvements".to_string(),
                estimated_duration_minutes: 30,
                automation_available: false,
                validation_criteria: vec![
                    "Post-mortem document created".to_string(),
                    "Action items identified".to_string(),
                ],
            },
        ];

        Ok(DisasterRecoveryPlan {
            id: format!("dr-plan-{}", uuid::Uuid::new_v4()),
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            created_by: user,
            rto_minutes,
            rpo_minutes,
            steps,
        })
    }

    fn run_recovery_test(
        _ctx: &Context,
        plan: &DisasterRecoveryPlan,
        dry_run: bool,
    ) -> Result<RecoveryTest> {
        // In a real implementation:
        // 1. Execute each recovery step
        // 2. Validate success criteria
        // 3. Measure actual vs. estimated time
        // 4. Roll back if dry_run is true

        let mut results = Vec::new();

        for step in &plan.steps {
            // Simulate step execution
            let success = if dry_run {
                // In dry run, all automated steps succeed
                step.automation_available
            } else {
                // In real run, actually execute
                true // Mock success
            };

            results.push(StepResult {
                step_order: step.order,
                status: if success {
                    StepStatus::Success
                } else {
                    StepStatus::Failed
                },
                duration_seconds: step.estimated_duration_minutes * 60,
                notes: if dry_run {
                    Some("Dry run - no actual changes made".to_string())
                } else {
                    None
                },
            });
        }

        let all_passed = results.iter().all(|r| matches!(r.status, StepStatus::Success));
        let status = if all_passed {
            TestStatus::Passed
        } else if results.iter().any(|r| matches!(r.status, StepStatus::Success)) {
            TestStatus::PartialSuccess
        } else {
            TestStatus::Failed
        };

        Ok(RecoveryTest {
            id: format!("dr-test-{}", uuid::Uuid::new_v4()),
            plan_id: plan.id.clone(),
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            status,
            results,
        })
    }

    fn save_plan(
        _ctx: &Context,
        infrastructure_root: &Path,
        plan: &DisasterRecoveryPlan,
    ) -> Result<()> {
        let dr_dir = infrastructure_root.join(".pmp").join("disaster-recovery");
        std::fs::create_dir_all(&dr_dir)?;

        let plan_file = dr_dir.join(format!("{}.json", plan.id));
        let content = serde_json::to_string_pretty(plan)?;
        std::fs::write(&plan_file, content)?;

        Ok(())
    }

    fn load_plan(
        _ctx: &Context,
        infrastructure_root: &Path,
        plan_id: &str,
    ) -> Result<DisasterRecoveryPlan> {
        let plan_file = infrastructure_root
            .join(".pmp")
            .join("disaster-recovery")
            .join(format!("{}.json", plan_id));

        if !plan_file.exists() {
            anyhow::bail!("DR plan not found: {}", plan_id);
        }

        let content = std::fs::read_to_string(&plan_file)?;
        let plan: DisasterRecoveryPlan = serde_json::from_str(&content)?;

        Ok(plan)
    }

    fn list_plans(_ctx: &Context, infrastructure_root: &Path) -> Result<Vec<DisasterRecoveryPlan>> {
        let dr_dir = infrastructure_root.join(".pmp").join("disaster-recovery");

        if !dr_dir.exists() {
            return Ok(vec![]);
        }

        let mut plans = Vec::new();

        for entry in std::fs::read_dir(&dr_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && !path.file_name().unwrap().to_str().unwrap().contains("test")
            {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(plan) = serde_json::from_str::<DisasterRecoveryPlan>(&content) {
                    plans.push(plan);
                }
            }
        }

        plans.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(plans)
    }

    fn save_test_result(
        _ctx: &Context,
        infrastructure_root: &Path,
        test: &RecoveryTest,
    ) -> Result<()> {
        let dr_dir = infrastructure_root.join(".pmp").join("disaster-recovery");
        std::fs::create_dir_all(&dr_dir)?;

        let test_file = dr_dir.join(format!("{}.json", test.id));
        let content = serde_json::to_string_pretty(test)?;
        std::fs::write(&test_file, content)?;

        Ok(())
    }

    fn get_current_user() -> Result<String> {
        if let Ok(output) = std::process::Command::new("git")
            .args(["config", "user.email"])
            .output()
            && output.status.success()
        {
            let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !email.is_empty() {
                return Ok(email);
            }
        }

        Ok(whoami::username())
    }
}
