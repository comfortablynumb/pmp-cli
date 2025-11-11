use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Handles the 'ui' command - starts HTTP server with web interface
pub struct UiCommand;

/// Shared application state
#[derive(Clone)]
struct AppState {
    ctx: Arc<crate::context::Context>,
}

/// Request/Response structures
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CreateProjectRequest {
    template_pack: String,
    template: String,
    environment: String,
    name: String,
    description: Option<String>,
    inputs: HashMap<String, serde_json::Value>,
    output: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenerateRequest {
    template_pack: String,
    template: String,
    #[allow(dead_code)]
    environment: Option<String>,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    inputs: HashMap<String, serde_json::Value>,
    output_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExecutorRequest {
    path: Option<String>,
    executor_args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DestroyRequest {
    path: Option<String>,
    yes: bool,
    executor_args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FindRequest {
    name: Option<String>,
    kind: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoadInfrastructureRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
struct InstallGitPackRequest {
    git_url: String,
}

#[derive(Debug, Deserialize)]
struct InstallLocalPackRequest {
    local_path: String,
}

#[derive(Debug, Deserialize)]
struct BrowseDirectoryRequest {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DirectoryEntry {
    name: String,
    path: String,
    is_dir: bool,
}

#[derive(Debug, Serialize)]
struct TemplatePackInfo {
    name: String,
    description: Option<String>,
    templates: Vec<TemplateInfo>,
}

#[derive(Debug, Serialize)]
struct TemplateInfo {
    name: String,
    description: Option<String>,
    kind: String,
    api_version: String,
    inputs: Vec<InputInfo>,
    environments: Vec<String>,
}

#[derive(Debug, Serialize)]
struct InputInfo {
    name: String,
    description: Option<String>,
    default: Option<serde_json::Value>,
    enum_values: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ProjectInfo {
    name: String,
    description: Option<String>,
    kind: String,
    path: String,
    environments: Vec<String>,
}

#[derive(Debug, Serialize)]
struct InfrastructureInfo {
    name: String,
    description: Option<String>,
    path: String,
    environments: Vec<String>,
    categories: Vec<CategoryInfo>,
}

#[derive(Debug, Serialize)]
struct CategoryInfo {
    id: String,
    name: String,
    description: Option<String>,
    templates: Vec<CategoryTemplateRef>,
    subcategories: Vec<CategoryInfo>,
}

#[derive(Debug, Serialize)]
struct CategoryTemplateRef {
    template_pack: String,
    template: String,
}

impl UiCommand {
    /// Execute the UI command
    pub fn execute(
        ctx: &crate::context::Context,
        port: Option<u16>,
        host: Option<String>,
    ) -> Result<()> {
        // Print startup message
        ctx.output.section("PMP Web UI");
        ctx.output.dimmed("Starting HTTP server...");

        let port = port.unwrap_or(8080);
        let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .context("Invalid host or port")?;

        // Create shared state
        let state = AppState {
            ctx: Arc::new(ctx.clone()),
        };

        // Build router
        let app = Router::new()
            // UI routes
            .route("/", get(serve_index))
            .route("/static/*path", get(serve_static))
            // API routes
            .route("/api/template-packs", get(list_template_packs))
            .route("/api/template-packs/:pack/templates", get(list_templates))
            .route(
                "/api/template-packs/:pack/templates/:template",
                get(get_template_details),
            )
            .route("/api/template-packs/install-git", post(install_git_pack))
            .route(
                "/api/template-packs/install-local",
                post(install_local_pack),
            )
            .route("/api/infrastructure", get(get_infrastructure))
            .route("/api/infrastructure/load", post(load_infrastructure))
            .route("/api/browse", post(browse_directory))
            .route("/api/projects", get(list_projects))
            .route("/api/projects/create", post(create_project))
            .route("/api/generate", post(generate))
            .route("/api/preview", post(preview))
            .route("/api/apply", post(apply))
            .route("/api/destroy", post(destroy))
            .route("/api/refresh", post(refresh))
            // CORS layer for development
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        ctx.output.blank();
        ctx.output
            .success(&format!("Server started at http://{}:{}", host, port));
        ctx.output.dimmed("Press Ctrl+C to stop");
        ctx.output.blank();

        // Run the server
        let runtime = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .context("Failed to bind to address")?;

            axum::serve(listener, app)
                .await
                .context("Failed to start server")?;

            Ok::<(), anyhow::Error>(())
        })?;

        Ok(())
    }
}

// ============================================================================
// UI Routes
// ============================================================================

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

async fn serve_static(Path(path): Path<String>) -> Response {
    match path.as_str() {
        "tailwind.css" => (
            StatusCode::OK,
            [("content-type", "text/css")],
            include_str!("../ui/tailwind.css"),
        )
            .into_response(),
        "jquery.js" => (
            StatusCode::OK,
            [("content-type", "application/javascript")],
            include_str!("../ui/jquery.js"),
        )
            .into_response(),
        "app.js" => (
            StatusCode::OK,
            [("content-type", "application/javascript")],
            include_str!("../ui/app.js"),
        )
            .into_response(),
        _ => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

// ============================================================================
// API Routes
// ============================================================================

async fn list_template_packs(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<TemplatePackInfo>>> {
    let template_packs_paths = params.get("template_packs_paths");

    // Parse custom paths
    let custom_paths: Vec<String> = template_packs_paths
        .map(|p| crate::template::discovery::parse_colon_separated_paths(p))
        .unwrap_or_default();
    let custom_paths_refs: Vec<&str> = custom_paths.iter().map(|s| s.as_str()).collect();

    // Discover template packs
    let result = crate::template::TemplateDiscovery::discover_template_packs_with_custom_paths(
        &*state.ctx.fs,
        &*state.ctx.output,
        &custom_paths_refs,
    );

    match result {
        Ok(packs) => {
            let mut pack_infos = Vec::new();
            for pack in packs {
                // Discover templates in this pack
                let templates_result =
                    crate::template::TemplateDiscovery::discover_templates_in_pack(
                        &*state.ctx.fs,
                        &*state.ctx.output,
                        &pack.path,
                    );

                let templates = match templates_result {
                    Ok(tmpl) => tmpl
                        .into_iter()
                        .map(|t| TemplateInfo {
                            name: t.resource.metadata.name.clone(),
                            description: t.resource.metadata.description.clone(),
                            kind: t.resource.spec.kind.clone(),
                            api_version: t.resource.spec.api_version.clone(),
                            inputs: t
                                .resource
                                .spec
                                .inputs
                                .iter()
                                .map(|i| InputInfo {
                                    name: i.name.clone(),
                                    description: i.description.clone(),
                                    default: i.default.clone(),
                                    enum_values: i.enum_values.clone(),
                                })
                                .collect(),
                            environments: t.resource.spec.environments.keys().cloned().collect(),
                        })
                        .collect(),
                    Err(_) => vec![],
                };

                pack_infos.push(TemplatePackInfo {
                    name: pack.resource.metadata.name.clone(),
                    description: pack.resource.metadata.description.clone(),
                    templates,
                });
            }

            Json(ApiResponse {
                success: true,
                data: Some(pack_infos),
                error: None,
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn list_templates(
    State(state): State<AppState>,
    Path(pack_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<Vec<TemplateInfo>>> {
    let template_packs_paths = params.get("template_packs_paths");

    // Parse custom paths
    let custom_paths: Vec<String> = template_packs_paths
        .map(|p| crate::template::discovery::parse_colon_separated_paths(p))
        .unwrap_or_default();
    let custom_paths_refs: Vec<&str> = custom_paths.iter().map(|s| s.as_str()).collect();

    // Discover template packs and find the requested one
    let packs_result =
        crate::template::TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*state.ctx.fs,
            &*state.ctx.output,
            &custom_paths_refs,
        );

    match packs_result {
        Ok(packs) => {
            let pack = packs
                .into_iter()
                .find(|p| p.resource.metadata.name == pack_name);

            match pack {
                Some(p) => {
                    let templates_result =
                        crate::template::TemplateDiscovery::discover_templates_in_pack(
                            &*state.ctx.fs,
                            &*state.ctx.output,
                            &p.path,
                        );

                    match templates_result {
                        Ok(templates) => {
                            let template_infos: Vec<TemplateInfo> = templates
                                .into_iter()
                                .map(|t| TemplateInfo {
                                    name: t.resource.metadata.name.clone(),
                                    description: t.resource.metadata.description.clone(),
                                    kind: t.resource.spec.kind.clone(),
                                    api_version: t.resource.spec.api_version.clone(),
                                    inputs: t
                                        .resource
                                        .spec
                                        .inputs
                                        .iter()
                                        .map(|i| InputInfo {
                                            name: i.name.clone(),
                                            description: i.description.clone(),
                                            default: i.default.clone(),
                                            enum_values: i.enum_values.clone(),
                                        })
                                        .collect(),
                                    environments: t
                                        .resource
                                        .spec
                                        .environments
                                        .keys()
                                        .cloned()
                                        .collect(),
                                })
                                .collect();

                            Json(ApiResponse {
                                success: true,
                                data: Some(template_infos),
                                error: None,
                            })
                        }
                        Err(e) => Json(ApiResponse {
                            success: false,
                            data: None,
                            error: Some(e.to_string()),
                        }),
                    }
                }
                None => Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Template pack '{}' not found", pack_name)),
                }),
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn get_template_details(
    State(state): State<AppState>,
    Path((pack_name, template_name)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<ApiResponse<TemplateInfo>> {
    let template_packs_paths = params.get("template_packs_paths");

    // Parse custom paths
    let custom_paths: Vec<String> = template_packs_paths
        .map(|p| crate::template::discovery::parse_colon_separated_paths(p))
        .unwrap_or_default();
    let custom_paths_refs: Vec<&str> = custom_paths.iter().map(|s| s.as_str()).collect();

    // Discover template packs and find the requested one
    let packs_result =
        crate::template::TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*state.ctx.fs,
            &*state.ctx.output,
            &custom_paths_refs,
        );

    match packs_result {
        Ok(packs) => {
            let pack = packs
                .into_iter()
                .find(|p| p.resource.metadata.name == pack_name);

            match pack {
                Some(p) => {
                    let templates_result =
                        crate::template::TemplateDiscovery::discover_templates_in_pack(
                            &*state.ctx.fs,
                            &*state.ctx.output,
                            &p.path,
                        );

                    match templates_result {
                        Ok(templates) => {
                            let template = templates
                                .into_iter()
                                .find(|t| t.resource.metadata.name == template_name);

                            match template {
                                Some(t) => {
                                    let template_info = TemplateInfo {
                                        name: t.resource.metadata.name.clone(),
                                        description: t.resource.metadata.description.clone(),
                                        kind: t.resource.spec.kind.clone(),
                                        api_version: t.resource.spec.api_version.clone(),
                                        inputs: t
                                            .resource
                                            .spec
                                            .inputs
                                            .iter()
                                            .map(|i| InputInfo {
                                                name: i.name.clone(),
                                                description: i.description.clone(),
                                                default: i.default.clone(),
                                                enum_values: i.enum_values.clone(),
                                            })
                                            .collect(),
                                        environments: t
                                            .resource
                                            .spec
                                            .environments
                                            .keys()
                                            .cloned()
                                            .collect(),
                                    };

                                    Json(ApiResponse {
                                        success: true,
                                        data: Some(template_info),
                                        error: None,
                                    })
                                }
                                None => Json(ApiResponse {
                                    success: false,
                                    data: None,
                                    error: Some(format!("Template '{}' not found", template_name)),
                                }),
                            }
                        }
                        Err(e) => Json(ApiResponse {
                            success: false,
                            data: None,
                            error: Some(e.to_string()),
                        }),
                    }
                }
                None => Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Template pack '{}' not found", pack_name)),
                }),
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn get_infrastructure(
    State(state): State<AppState>,
) -> Json<ApiResponse<InfrastructureInfo>> {
    // Try to load infrastructure from current directory
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to get current directory: {}", e)),
            });
        }
    };

    let infra_path = current_dir.join(".pmp.infrastructure.yaml");

    if !state.ctx.fs.exists(&infra_path) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some("No infrastructure found in current directory".to_string()),
        });
    }

    match crate::template::metadata::InfrastructureResource::from_file(&*state.ctx.fs, &infra_path)
    {
        Ok(infra) => {
            let categories: Vec<CategoryInfo> =
                infra.spec.categories.iter().map(convert_category).collect();

            let info = InfrastructureInfo {
                name: infra.metadata.name.clone(),
                description: infra.metadata.description.clone(),
                path: current_dir.to_string_lossy().to_string(),
                environments: infra.spec.environments.keys().cloned().collect(),
                categories,
            };

            Json(ApiResponse {
                success: true,
                data: Some(info),
                error: None,
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

fn convert_category(category: &crate::template::metadata::Category) -> CategoryInfo {
    CategoryInfo {
        id: category.id.clone(),
        name: category.name.clone(),
        description: category.description.clone(),
        templates: category
            .templates
            .iter()
            .map(|t| CategoryTemplateRef {
                template_pack: t.template_pack.clone(),
                template: t.template.clone(),
            })
            .collect(),
        subcategories: category
            .subcategories
            .iter()
            .map(convert_category)
            .collect(),
    }
}

async fn list_projects(
    State(state): State<AppState>,
    Query(params): Query<FindRequest>,
) -> Json<ApiResponse<Vec<ProjectInfo>>> {
    use std::path::PathBuf;

    // Use provided path or current directory
    let base_dir = if let Some(ref path_str) = params.path {
        PathBuf::from(path_str)
    } else {
        match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                return Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to get current directory: {}", e)),
                });
            }
        }
    };

    let projects_dir = base_dir.join("projects");

    if !state.ctx.fs.exists(&projects_dir) {
        return Json(ApiResponse {
            success: true,
            data: Some(vec![]),
            error: None,
        });
    }

    match crate::collection::CollectionDiscovery::discover_projects(
        &*state.ctx.fs,
        &*state.ctx.output,
        &base_dir,
    ) {
        Ok(projects) => {
            let mut project_infos: Vec<ProjectInfo> = Vec::new();

            for p in projects {
                // Filter by name if provided
                if let Some(ref name_filter) = params.name
                    && !p.name.to_lowercase().contains(&name_filter.to_lowercase())
                {
                    continue;
                }

                // Filter by kind if provided
                if let Some(ref kind_filter) = params.kind
                    && p.kind != *kind_filter
                {
                    continue;
                }

                // Discover environments for this project
                let project_path = base_dir.join(&p.path);
                let environments = crate::collection::CollectionDiscovery::discover_environments(
                    &*state.ctx.fs,
                    &project_path,
                )
                .unwrap_or_default();

                project_infos.push(ProjectInfo {
                    name: p.name.clone(),
                    description: None,
                    kind: p.kind.clone(),
                    path: p.path.clone(),
                    environments,
                });
            }

            project_infos.sort_by(|a, b| a.name.cmp(&b.name));

            Json(ApiResponse {
                success: true,
                data: Some(project_infos),
                error: None,
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn create_project(
    State(_state): State<AppState>,
    Json(_req): Json<CreateProjectRequest>,
) -> Json<ApiResponse<String>> {
    // This is a simplified version - in a real implementation, you'd want to
    // call the CreateCommand::execute function with appropriate parameters
    // For now, return an error indicating manual implementation needed
    Json(ApiResponse {
        success: false,
        data: None,
        error: Some(
            "Project creation via API not yet fully implemented. Use CLI for now.".to_string(),
        ),
    })
}

async fn generate(
    State(state): State<AppState>,
    Json(req): Json<GenerateRequest>,
) -> Json<ApiResponse<String>> {
    // Call the GenerateCommand with the provided parameters
    let result = crate::commands::GenerateCommand::execute(
        &state.ctx,
        Some(&req.template_pack),
        Some(&req.template),
        req.output_dir.as_deref(),
        None,
    );

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some("Files generated successfully".to_string()),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn preview(
    State(state): State<AppState>,
    Json(req): Json<ExecutorRequest>,
) -> Json<ApiResponse<String>> {
    let result = crate::commands::PreviewCommand::execute(
        &state.ctx,
        req.path.as_deref(),
        &req.executor_args,
    );

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some("Preview completed successfully".to_string()),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn apply(
    State(state): State<AppState>,
    Json(req): Json<ExecutorRequest>,
) -> Json<ApiResponse<String>> {
    let result =
        crate::commands::ApplyCommand::execute(&state.ctx, req.path.as_deref(), &req.executor_args);

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some("Apply completed successfully".to_string()),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn destroy(
    State(state): State<AppState>,
    Json(req): Json<DestroyRequest>,
) -> Json<ApiResponse<String>> {
    let result = crate::commands::DestroyCommand::execute(
        &state.ctx,
        req.path.as_deref(),
        req.yes,
        &req.executor_args,
    );

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some("Destroy completed successfully".to_string()),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<ExecutorRequest>,
) -> Json<ApiResponse<String>> {
    let result = crate::commands::RefreshCommand::execute(
        &state.ctx,
        req.path.as_deref(),
        &req.executor_args,
    );

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some("Refresh completed successfully".to_string()),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

async fn browse_directory(
    State(state): State<AppState>,
    Json(req): Json<BrowseDirectoryRequest>,
) -> Json<ApiResponse<Vec<DirectoryEntry>>> {
    use std::path::PathBuf;

    // Default to home directory if no path provided
    let base_path = if let Some(ref path_str) = req.path {
        PathBuf::from(path_str)
    } else {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    };

    // Verify the path exists and is a directory
    if !state.ctx.fs.exists(&base_path) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Path does not exist: {}", base_path.display())),
        });
    }

    if !state.ctx.fs.is_dir(&base_path) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Path is not a directory: {}", base_path.display())),
        });
    }

    // Read directory entries
    match state.ctx.fs.read_dir(&base_path) {
        Ok(entries) => {
            let mut directory_entries = Vec::new();

            // Add parent directory entry if not at root
            if let Some(parent) = base_path.parent() {
                directory_entries.push(DirectoryEntry {
                    name: "..".to_string(),
                    path: parent.to_string_lossy().to_string(),
                    is_dir: true,
                });
            }

            for path in entries {
                let is_dir = state.ctx.fs.is_dir(&path);

                // Only show directories
                if is_dir && let Some(name) = path.file_name() {
                    directory_entries.push(DirectoryEntry {
                        name: name.to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                        is_dir,
                    });
                }
            }

            // Sort directories alphabetically
            directory_entries.sort_by(|a, b| {
                if a.name == ".." {
                    std::cmp::Ordering::Less
                } else if b.name == ".." {
                    std::cmp::Ordering::Greater
                } else {
                    a.name.cmp(&b.name)
                }
            });

            Json(ApiResponse {
                success: true,
                data: Some(directory_entries),
                error: None,
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to read directory: {}", e)),
        }),
    }
}

async fn load_infrastructure(
    State(state): State<AppState>,
    Json(req): Json<LoadInfrastructureRequest>,
) -> Json<ApiResponse<InfrastructureInfo>> {
    use std::path::PathBuf;

    let infra_path = PathBuf::from(&req.path);

    // Check if path is a directory - if so, look for .pmp.infrastructure.yaml inside
    let yaml_path = if state.ctx.fs.is_dir(&infra_path) {
        infra_path.join(".pmp.infrastructure.yaml")
    } else {
        infra_path.clone()
    };

    // Verify the file exists
    if !state.ctx.fs.exists(&yaml_path) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!(
                "Infrastructure file not found at: {}",
                yaml_path.display()
            )),
        });
    }

    // Load the infrastructure
    match crate::template::metadata::InfrastructureResource::from_file(&*state.ctx.fs, &yaml_path) {
        Ok(infra) => {
            let categories: Vec<CategoryInfo> =
                infra.spec.categories.iter().map(convert_category).collect();

            // Get the directory path of the infrastructure
            let infra_dir = if state.ctx.fs.is_dir(&infra_path) {
                infra_path.to_string_lossy().to_string()
            } else {
                infra_path
                    .parent()
                    .unwrap_or(&infra_path)
                    .to_string_lossy()
                    .to_string()
            };

            let info = InfrastructureInfo {
                name: infra.metadata.name.clone(),
                description: infra.metadata.description.clone(),
                path: infra_dir,
                environments: infra.spec.environments.keys().cloned().collect(),
                categories,
            };

            Json(ApiResponse {
                success: true,
                data: Some(info),
                error: None,
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to load infrastructure: {}", e)),
        }),
    }
}

async fn install_git_pack(
    State(state): State<AppState>,
    Json(req): Json<InstallGitPackRequest>,
) -> Json<ApiResponse<String>> {
    // Validate Git URL
    if req.git_url.is_empty() {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Git URL cannot be empty".to_string()),
        });
    }

    // Extract repository name from URL (e.g., https://github.com/user/repo.git -> repo)
    let repo_name = req
        .git_url
        .trim_end_matches(".git")
        .split('/')
        .next_back()
        .unwrap_or("template-pack")
        .to_string();

    // Get home directory for template packs
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Could not determine home directory".to_string()),
            });
        }
    };

    let template_packs_dir = home_dir.join(".pmp").join("template-packs");
    let target_path = template_packs_dir.join(&repo_name);

    // Create template packs directory if it doesn't exist
    if !state.ctx.fs.exists(&template_packs_dir)
        && let Err(e) = state.ctx.fs.create_dir_all(&template_packs_dir)
    {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to create template packs directory: {}", e)),
        });
    }

    // Check if pack already exists
    if state.ctx.fs.exists(&target_path) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!(
                "Template pack '{}' already exists at {}. Please remove it first or use a different name.",
                repo_name,
                target_path.display()
            )),
        });
    }

    // Clone the repository
    let output = state.ctx.command.execute(
        "git",
        &["clone", &req.git_url, target_path.to_str().unwrap()],
        &template_packs_dir,
    );

    match output {
        Ok(result) => {
            if result.status.success() {
                // Verify that .pmp.template-pack.yaml exists
                let pack_yaml = target_path.join(".pmp.template-pack.yaml");
                if !state.ctx.fs.exists(&pack_yaml) {
                    // Cleanup the cloned directory
                    let _ = state.ctx.fs.remove_dir_all(&target_path);
                    return Json(ApiResponse {
                        success: false,
                        data: None,
                        error: Some(
                            "Cloned repository is not a valid template pack (missing .pmp.template-pack.yaml)"
                                .to_string(),
                        ),
                    });
                }

                Json(ApiResponse {
                    success: true,
                    data: Some(format!(
                        "Successfully cloned template pack '{}' to {}",
                        repo_name,
                        target_path.display()
                    )),
                    error: None,
                })
            } else {
                let error_msg = String::from_utf8_lossy(&result.stderr).to_string();
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Git clone failed: {}", error_msg)),
                })
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to execute git clone: {}", e)),
        }),
    }
}

async fn install_local_pack(
    State(state): State<AppState>,
    Json(req): Json<InstallLocalPackRequest>,
) -> Json<ApiResponse<String>> {
    use std::path::PathBuf;

    let local_path = PathBuf::from(&req.local_path);

    // Verify the path exists
    if !state.ctx.fs.exists(&local_path) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Path does not exist: {}", local_path.display())),
        });
    }

    // Verify it's a directory
    if !state.ctx.fs.is_dir(&local_path) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Path is not a directory: {}", local_path.display())),
        });
    }

    // Verify .pmp.template-pack.yaml exists
    let pack_yaml = local_path.join(".pmp.template-pack.yaml");
    if !state.ctx.fs.exists(&pack_yaml) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(
                "Directory is not a valid template pack (missing .pmp.template-pack.yaml)"
                    .to_string(),
            ),
        });
    }

    // Try to load the template pack to validate it
    match crate::template::metadata::TemplatePackResource::from_file(&*state.ctx.fs, &pack_yaml) {
        Ok(pack) => Json(ApiResponse {
            success: true,
            data: Some(format!(
                "Successfully loaded template pack '{}' from {}",
                pack.metadata.name,
                local_path.display()
            )),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to load template pack: {}", e)),
        }),
    }
}
