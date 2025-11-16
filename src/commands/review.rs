use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct ReviewCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub id: String,
    pub project: String,
    pub environment: String,
    pub requester: String,
    pub created_at: String,
    pub description: String,
    pub changes_summary: String,
    pub status: ReviewStatus,
    pub approvals: Vec<Approval>,
    pub required_approvals: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ReviewStatus {
    Pending,
    Approved,
    ChangesRequested,
    Rejected,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Approval {
    pub reviewer: String,
    pub approved_at: String,
    pub decision: ApprovalDecision,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Approve,
    RequestChanges,
    Reject,
}

impl ReviewCommand {
    pub fn execute_request(ctx: &Context, path: Option<&str>, description: Option<&str>) -> Result<()> {
        ctx.output.section("Request Peer Review");
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

        // Get description
        let desc = if let Some(d) = description {
            d.to_string()
        } else {
            ctx.input.text("Description of changes:", None)?
        };

        // Get required approvals
        let required: usize = ctx
            .input
            .text("Required approvals:", Some("1"))?
            .parse()
            .unwrap_or(1);

        // Get current user
        let user = Self::get_current_user()?;

        // Generate changes summary
        let changes_summary = Self::generate_changes_summary(ctx, &current_path)?;

        // Create review request
        let review = ReviewRequest {
            id: format!("review-{}", uuid::Uuid::new_v4()),
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            requester: user.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            description: desc,
            changes_summary,
            status: ReviewStatus::Pending,
            approvals: vec![],
            required_approvals: required,
        };

        // Save review request
        Self::save_review(ctx, &infrastructure_root, &review)?;

        ctx.output.success("Review request created");
        ctx.output.dimmed(&format!("Review ID: {}", review.id));
        ctx.output.dimmed(&format!("Required approvals: {}", required));

        Ok(())
    }

    pub fn execute_approve(
        ctx: &Context,
        review_id: Option<&str>,
        decision: &str,
        comment: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Approve Review Request");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Get review ID
        let id = if let Some(r) = review_id {
            r.to_string()
        } else {
            // List pending reviews and let user select
            let pending = Self::get_pending_reviews(ctx, &infrastructure_root)?;
            if pending.is_empty() {
                ctx.output.info("No pending review requests");
                return Ok(());
            }

            ctx.output.subsection("Pending Reviews");
            output::blank();

            let options: Vec<String> = pending
                .iter()
                .map(|r| format!("{} - {}/{}", r.id, r.project, r.environment))
                .collect();

            let selection = ctx.input.select("Select review:", options.clone())?;
            let idx = options.iter().position(|x| x == &selection).unwrap_or(0);
            pending[idx].id.clone()
        };

        // Load review
        let mut review = Self::load_review(ctx, &infrastructure_root, &id)?;

        ctx.output.key_value("Review ID", &review.id);
        ctx.output.key_value("Project", &review.project);
        ctx.output.key_value("Environment", &review.environment);
        ctx.output.key_value("Requester", &review.requester);
        ctx.output.key_value("Description", &review.description);
        output::blank();

        // Show changes
        ctx.output.subsection("Changes Summary");
        ctx.output.dimmed(&review.changes_summary);
        output::blank();

        // Parse decision
        let approval_decision = match decision.to_lowercase().as_str() {
            "approve" => ApprovalDecision::Approve,
            "request-changes" => ApprovalDecision::RequestChanges,
            "reject" => ApprovalDecision::Reject,
            _ => {
                anyhow::bail!("Invalid decision. Use: approve, request-changes, or reject");
            }
        };

        // Get current user
        let user = Self::get_current_user()?;

        // Check if user already approved
        if review.approvals.iter().any(|a| a.reviewer == user) {
            ctx.output.warning("You have already reviewed this request");
            return Ok(());
        }

        // Add approval
        let approval = Approval {
            reviewer: user.clone(),
            approved_at: chrono::Utc::now().to_rfc3339(),
            decision: approval_decision,
            comment: comment.map(String::from),
        };

        review.approvals.push(approval);

        // Update status
        let approve_count = review
            .approvals
            .iter()
            .filter(|a| matches!(a.decision, ApprovalDecision::Approve))
            .count();

        if approve_count >= review.required_approvals {
            review.status = ReviewStatus::Approved;
        } else if review
            .approvals
            .iter()
            .any(|a| matches!(a.decision, ApprovalDecision::Reject))
        {
            review.status = ReviewStatus::Rejected;
        } else if review
            .approvals
            .iter()
            .any(|a| matches!(a.decision, ApprovalDecision::RequestChanges))
        {
            review.status = ReviewStatus::ChangesRequested;
        }

        // Save review
        Self::save_review(ctx, &infrastructure_root, &review)?;

        ctx.output.success("Review submitted");
        ctx.output.dimmed(&format!("Status: {:?}", review.status));
        ctx.output.dimmed(&format!(
            "Approvals: {}/{}",
            approve_count, review.required_approvals
        ));

        Ok(())
    }

    pub fn execute_list(ctx: &Context, status_filter: Option<&str>) -> Result<()> {
        ctx.output.section("Review Requests");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let reviews = Self::get_all_reviews(ctx, &infrastructure_root)?;

        // Filter by status if specified
        let filtered: Vec<_> = if let Some(status) = status_filter {
            reviews
                .into_iter()
                .filter(|r| {
                    matches!(
                        (status.to_lowercase().as_str(), &r.status),
                        ("pending", ReviewStatus::Pending)
                            | ("approved", ReviewStatus::Approved)
                            | ("rejected", ReviewStatus::Rejected)
                            | ("changes-requested", ReviewStatus::ChangesRequested)
                    )
                })
                .collect()
        } else {
            reviews
        };

        if filtered.is_empty() {
            ctx.output.info("No review requests found");
            return Ok(());
        }

        for review in &filtered {
            let status_icon = match review.status {
                ReviewStatus::Pending => "⏳",
                ReviewStatus::Approved => "✓",
                ReviewStatus::ChangesRequested => "🔄",
                ReviewStatus::Rejected => "✗",
            };

            ctx.output.dimmed(&format!("{} [{}] {:?}", status_icon, review.id, review.status));
            ctx.output.dimmed(&format!("  {}/{}", review.project, review.environment));
            ctx.output.dimmed(&format!("  By: {}", review.requester));
            ctx.output.dimmed(&format!("  {}", review.description));
            ctx.output.dimmed(&format!(
                "  Approvals: {}/{}",
                review.approvals.len(),
                review.required_approvals
            ));
            output::blank();
        }

        ctx.output.success(&format!("{} review requests", filtered.len()));

        Ok(())
    }

    // Helper functions

    fn get_current_user() -> Result<String> {
        // Try to get user from git config
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

    fn generate_changes_summary(_ctx: &Context, _env_path: &Path) -> Result<String> {
        // In a real implementation:
        // 1. Run terraform plan
        // 2. Parse the output
        // 3. Generate a summary of changes

        Ok("2 resources to add, 3 resources to modify, 0 resources to destroy".to_string())
    }

    fn save_review(
        _ctx: &Context,
        infrastructure_root: &Path,
        review: &ReviewRequest,
    ) -> Result<()> {
        let reviews_dir = infrastructure_root.join(".pmp").join("reviews");
        std::fs::create_dir_all(&reviews_dir)?;

        let review_file = reviews_dir.join(format!("{}.json", review.id));
        let content = serde_json::to_string_pretty(review)?;
        std::fs::write(&review_file, content)?;

        Ok(())
    }

    fn load_review(
        _ctx: &Context,
        infrastructure_root: &Path,
        review_id: &str,
    ) -> Result<ReviewRequest> {
        let review_file = infrastructure_root
            .join(".pmp")
            .join("reviews")
            .join(format!("{}.json", review_id));

        let content = std::fs::read_to_string(&review_file)?;
        let review: ReviewRequest = serde_json::from_str(&content)?;

        Ok(review)
    }

    fn get_pending_reviews(
        ctx: &Context,
        infrastructure_root: &Path,
    ) -> Result<Vec<ReviewRequest>> {
        let all = Self::get_all_reviews(ctx, infrastructure_root)?;
        Ok(all
            .into_iter()
            .filter(|r| matches!(r.status, ReviewStatus::Pending))
            .collect())
    }

    fn get_all_reviews(_ctx: &Context, infrastructure_root: &Path) -> Result<Vec<ReviewRequest>> {
        let reviews_dir = infrastructure_root.join(".pmp").join("reviews");

        if !reviews_dir.exists() {
            return Ok(vec![]);
        }

        let mut reviews = Vec::new();

        for entry in std::fs::read_dir(&reviews_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(review) = serde_json::from_str::<ReviewRequest>(&content) {
                    reviews.push(review);
                }
            }
        }

        // Sort by created_at (newest first)
        reviews.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(reviews)
    }
}
