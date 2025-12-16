use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{
        Json, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tower_http::cors::{Any, CorsLayer};

/// Handles the 'ui' command - starts HTTP server with web interface
pub struct UiCommand;

/// Operation status for tracking running operations
#[derive(Debug, Clone, Serialize)]
pub struct OperationStatus {
    pub id: String,
    pub operation: String,
    pub project_path: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub success: Option<bool>,
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    ctx: Arc<crate::context::Context>,
    operations: Arc<TokioMutex<HashMap<String, OperationStatus>>>,
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

/// WebSocket operation request
#[derive(Debug, Deserialize)]
struct WsOperationRequest {
    operation: String,
    path: Option<String>,
    executor_args: Option<Vec<String>>,
    yes: Option<bool>,
}

/// WebSocket message types
#[derive(Debug, Serialize)]
struct WsMessage {
    #[serde(rename = "type")]
    msg_type: String,
    data: serde_json::Value,
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
struct DriveInfo {
    name: String,
    path: String,
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
    #[serde(rename = "type")]
    input_type: String,
    description: Option<String>,
    default: Option<serde_json::Value>,
    required: bool,
    /// For select/multiselect types
    options: Option<Vec<SelectOption>>,
    /// For number type
    min: Option<f64>,
    max: Option<f64>,
    /// Conditional visibility conditions
    conditions: Option<Vec<InputConditionInfo>>,
}

#[derive(Debug, Serialize)]
struct SelectOption {
    label: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct InputConditionInfo {
    field: String,
    condition: String,
    value: Option<serde_json::Value>,
}

/// Convert an InputDefinition to InputInfo for API response
fn convert_input_to_info(input: &crate::template::metadata::InputDefinition) -> InputInfo {
    use crate::template::metadata::InputType;

    // Determine input type string and extract type-specific fields
    let (type_str, options, min, max) = match &input.input_type {
        Some(InputType::String) => ("string".to_string(), None, None, None),
        Some(InputType::Boolean) => ("boolean".to_string(), None, None, None),
        Some(InputType::Number { min, max, .. }) => {
            ("number".to_string(), None, *min, *max)
        }
        Some(InputType::Select { options }) => {
            let opts: Vec<SelectOption> = options
                .iter()
                .map(|o| SelectOption {
                    label: o.label.clone(),
                    value: o.value.clone(),
                })
                .collect();
            ("select".to_string(), Some(opts), None, None)
        }
        Some(InputType::MultiSelect { options, .. }) => {
            let opts: Vec<SelectOption> = options
                .iter()
                .map(|o| SelectOption {
                    label: o.label.clone(),
                    value: o.value.clone(),
                })
                .collect();
            ("multiselect".to_string(), Some(opts), None, None)
        }
        Some(InputType::Password) => ("password".to_string(), None, None, None),
        Some(InputType::Path { .. }) => ("path".to_string(), None, None, None),
        Some(InputType::Url { .. }) => ("url".to_string(), None, None, None),
        Some(InputType::Json { .. }) => ("json".to_string(), None, None, None),
        Some(InputType::Yaml { .. }) => ("yaml".to_string(), None, None, None),
        Some(InputType::Email) => ("email".to_string(), None, None, None),
        Some(InputType::IpAddress { .. }) => ("ip".to_string(), None, None, None),
        Some(InputType::Cidr { .. }) => ("cidr".to_string(), None, None, None),
        Some(InputType::Port) => ("port".to_string(), None, None, None),
        Some(InputType::Duration { .. }) => ("duration".to_string(), None, None, None),
        Some(InputType::Cron) => ("cron".to_string(), None, None, None),
        Some(InputType::Semver { .. }) => ("semver".to_string(), None, None, None),
        Some(InputType::Color { .. }) => ("color".to_string(), None, None, None),
        Some(InputType::KeyValue { .. }) => ("keyvalue".to_string(), None, None, None),
        Some(InputType::List { .. }) => ("list".to_string(), None, None, None),
        Some(InputType::Object { .. }) => ("object".to_string(), None, None, None),
        Some(InputType::RepeatableObject { .. }) => ("repeatable_object".to_string(), None, None, None),
        Some(InputType::Date { .. }) => ("date".to_string(), None, None, None),
        Some(InputType::DateTime { .. }) => ("datetime".to_string(), None, None, None),
        Some(InputType::ProjectSelect { .. }) => ("project_select".to_string(), None, None, None),
        Some(InputType::MultiProjectSelect { .. }) => ("multi_project_select".to_string(), None, None, None),
        None => {
            // Handle deprecated enum_values or default to string
            if input.enum_values.is_some() {
                let opts: Vec<SelectOption> = input
                    .enum_values
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|v| SelectOption {
                        label: v.clone(),
                        value: v.clone(),
                    })
                    .collect();
                ("select".to_string(), Some(opts), None, None)
            } else {
                ("string".to_string(), None, None, None)
            }
        }
    };

    // Convert conditions (simplified - InputCondition uses input_name and equals)
    let conditions = if input.conditions.is_empty() {
        None
    } else {
        Some(
            input
                .conditions
                .iter()
                .map(|c| InputConditionInfo {
                    field: c.input_name.clone(),
                    condition: if c.equals.is_some() {
                        "equals".to_string()
                    } else {
                        "exists".to_string()
                    },
                    value: c.equals.clone(),
                })
                .collect(),
        )
    };

    InputInfo {
        name: input.name.clone(),
        input_type: type_str,
        description: input.description.clone(),
        default: input.default.clone(),
        required: input.default.is_none(),
        options,
        min,
        max,
        conditions,
    }
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

        // Check for infrastructure in current directory
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        let infra_path = current_dir.join(".pmp.infrastructure.yaml");

        if !ctx.fs.exists(&infra_path) {
            ctx.output
                .error("No infrastructure project found in current directory");
            ctx.output
                .dimmed(&format!("Expected file: {}", infra_path.display()));
            ctx.output.blank();
            ctx.output
                .dimmed("The UI command must be run from an infrastructure project directory.");
            ctx.output.dimmed("Create one first using 'pmp init' or navigate to an existing infrastructure project.");
            return Err(anyhow::anyhow!("No infrastructure project found"));
        }

        // Verify it's a valid infrastructure file
        crate::template::metadata::InfrastructureResource::from_file(&*ctx.fs, &infra_path)
            .context("Failed to load infrastructure file")?;

        ctx.output
            .success(&format!("Found infrastructure: {}", infra_path.display()));
        ctx.output.dimmed("Starting HTTP server...");

        let port = port.unwrap_or(8080);
        let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .context("Invalid host or port")?;

        // Create shared state
        let state = AppState {
            ctx: Arc::new(ctx.clone()),
            operations: Arc::new(TokioMutex::new(HashMap::new())),
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
            .route("/api/drives", get(list_drives))
            .route("/api/projects", get(list_projects))
            .route("/api/projects/create", post(create_project))
            .route("/api/generate", post(generate))
            .route("/api/preview", post(preview))
            .route("/api/apply", post(apply))
            .route("/api/destroy", post(destroy))
            .route("/api/refresh", post(refresh))
            .route("/api/graph", get(get_dependency_graph))
            // WebSocket route for streaming operations
            .route("/ws/execute", get(ws_execute_handler))
            // Dashboard API routes
            .route("/api/dashboard", get(get_dashboard))
            .route("/api/operations", get(list_operations))
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
                                .map(convert_input_to_info)
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
                                        .map(convert_input_to_info)
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
                                            .map(convert_input_to_info)
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
                    path: project_path.to_string_lossy().to_string(),
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
    State(state): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> Json<ApiResponse<String>> {
    use crate::traits::output::MockOutput;
    use std::sync::Arc;

    // Validate required fields
    if req.template_pack.is_empty() {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some("template_pack is required".to_string()),
        });
    }

    if req.template.is_empty() {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some("template is required".to_string()),
        });
    }

    if req.environment.is_empty() {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some("environment is required".to_string()),
        });
    }

    if req.name.is_empty() {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some("name is required".to_string()),
        });
    }

    // Validate project name format using schema validator
    if let Err(e) = crate::schema::validator::SchemaValidator::validate_project_name(&req.name) {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Invalid project name: {}", e)),
        });
    }

    // Create buffered output to capture command output
    let buffered_output = Arc::new(MockOutput::new());

    // Create a temporary context with buffered output
    let mut temp_ctx = (*state.ctx).clone();
    temp_ctx.output = buffered_output.clone();

    // Convert inputs to JSON string format for CreateCommand
    // Format: key1=value1,key2=value2
    let inputs_str = if !req.inputs.is_empty() {
        let inputs_vec: Vec<String> = req
            .inputs
            .iter()
            .map(|(k, v)| {
                let value_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => v.to_string(),
                };
                format!("{}={}", k, value_str)
            })
            .collect();
        Some(inputs_vec.join(","))
    } else {
        None
    };

    // Build template spec string: "template_pack/template"
    let template_spec = format!("{}/{}", req.template_pack, req.template);

    // Execute project creation with all parameters specified to minimize interactivity
    // Note: CreateCommand supports non-interactive mode when template_spec, project_name,
    // and environment_name are all provided
    let result = crate::commands::CreateCommand::execute(
        &temp_ctx,
        req.output.as_deref(),        // output_path
        None,                          // template_packs_paths
        inputs_str.as_deref(),         // inputs_str (predefined inputs)
        Some(&template_spec),          // template_spec (pack/template)
        false,                         // auto_apply
        Some(&req.name),               // project_name
        Some(&req.environment),        // environment_name
    );

    // Get captured output
    let output_text = buffered_output.to_text();

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some(format!(
                "Project '{}' created successfully in environment '{}'\n\n{}",
                req.name, req.environment, output_text
            )),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: Some(output_text),
            error: Some(e.to_string()),
        }),
    }
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

/// Response structure for dependency graph
#[derive(Debug, Serialize)]
struct GraphResponse {
    /// Mermaid diagram code
    mermaid: String,
    /// List of nodes in the graph
    nodes: Vec<GraphNode>,
    /// List of edges (dependencies)
    edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
struct GraphNode {
    id: String,
    project_name: String,
    environment: String,
    kind: Option<String>,
}

#[derive(Debug, Serialize)]
struct GraphEdge {
    from: String,
    to: String,
}

async fn get_dependency_graph(
    State(state): State<AppState>,
) -> Json<ApiResponse<GraphResponse>> {
    use crate::collection::CollectionDiscovery;
    use std::collections::{HashMap, HashSet};

    // Find infrastructure
    let collection_result = CollectionDiscovery::find_collection(&*state.ctx.fs);

    let (_infrastructure, infrastructure_root) = match collection_result {
        Ok(Some((infra, root))) => (infra, root),
        Ok(None) => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some("No infrastructure found".to_string()),
            });
        }
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            });
        }
    };

    // Discover all projects
    let projects = match CollectionDiscovery::discover_projects(
        &*state.ctx.fs,
        &*state.ctx.output,
        &infrastructure_root,
    ) {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to discover projects: {}", e)),
            });
        }
    };

    // Build graph data
    let mut all_nodes: HashSet<String> = HashSet::new();
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    let mut node_info: HashMap<String, GraphNode> = HashMap::new();

    for project in &projects {
        let project_path = infrastructure_root.join(&project.path);
        let environments_dir = project_path.join("environments");

        if let Ok(env_entries) = state.ctx.fs.read_dir(&environments_dir) {
            for env_path in env_entries {
                let env_file = env_path.join(".pmp.environment.yaml");

                if state.ctx.fs.exists(&env_file)
                    && let Ok(env_resource) =
                        crate::template::metadata::DynamicProjectEnvironmentResource::from_file(
                            &*state.ctx.fs,
                            &env_file,
                        )
                {
                    let env_name = env_resource.metadata.environment_name.clone();
                    let node_key = format!("{}:{}", project.name, env_name);

                    all_nodes.insert(node_key.clone());
                    node_info.insert(
                        node_key.clone(),
                        GraphNode {
                            id: node_key.clone(),
                            project_name: project.name.clone(),
                            environment: env_name.clone(),
                            kind: Some(env_resource.kind.clone()),
                        },
                    );

                    // Extract dependencies
                    for dep in &env_resource.spec.dependencies {
                        for dep_env in &dep.project.environments {
                            let dep_key = format!("{}:{}", dep.project.name, dep_env);
                            dependencies
                                .entry(node_key.clone())
                                .or_default()
                                .push(dep_key);
                        }
                    }
                }
            }
        }
    }

    // Generate Mermaid diagram
    let mut mermaid = String::from("graph TD\n");

    let mut sorted_nodes: Vec<_> = all_nodes.iter().collect();
    sorted_nodes.sort();

    for node_key in &sorted_nodes {
        let sanitized_id = node_key.replace([':', '-', '.', ' '], "_");
        if let Some(info) = node_info.get(*node_key) {
            let label = format!("{}\\n({})", info.project_name, info.environment);
            mermaid.push_str(&format!("    {}[\"{}\"]\n", sanitized_id, label));
        }
    }

    mermaid.push('\n');

    for (parent_key, deps) in &dependencies {
        let parent_id = parent_key.replace([':', '-', '.', ' '], "_");
        for dep in deps {
            let dep_id = dep.replace([':', '-', '.', ' '], "_");
            mermaid.push_str(&format!("    {} --> {}\n", parent_id, dep_id));
        }
    }

    // Build edges list
    let edges: Vec<GraphEdge> = dependencies
        .iter()
        .flat_map(|(from, to_list)| {
            to_list.iter().map(move |to| GraphEdge {
                from: from.clone(),
                to: to.clone(),
            })
        })
        .collect();

    // Build nodes list
    let nodes: Vec<GraphNode> = node_info.into_values().collect();

    Json(ApiResponse {
        success: true,
        data: Some(GraphResponse {
            mermaid,
            nodes,
            edges,
        }),
        error: None,
    })
}

async fn preview(
    State(state): State<AppState>,
    Json(req): Json<ExecutorRequest>,
) -> Json<ApiResponse<String>> {
    use crate::traits::output::MockOutput;
    use std::sync::Arc;

    // Create a buffered output to capture command output
    let buffered_output = Arc::new(MockOutput::new());

    // Create a temporary context with buffered output
    let mut temp_ctx = (*state.ctx).clone();
    temp_ctx.output = buffered_output.clone();

    let result = crate::commands::PreviewCommand::execute(
        &temp_ctx,
        req.path.as_deref(),
        false,  // show_cost - not supported in UI yet
        false,  // skip_policy - run validation in UI
        None,   // parallel - not supported in UI yet
        false,  // show_diff - not supported in UI yet
        "ascii", // diff_format
        false,  // side_by_side
        None,   // diff_output
        false,  // show_unchanged
        false,  // show_sensitive
        &req.executor_args,
    );

    // Get captured output
    let output_text = buffered_output.to_text();

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some(output_text),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: Some(output_text),
            error: Some(e.to_string()),
        }),
    }
}

async fn apply(
    State(state): State<AppState>,
    Json(req): Json<ExecutorRequest>,
) -> Json<ApiResponse<String>> {
    use crate::traits::output::MockOutput;
    use std::sync::Arc;

    let buffered_output = Arc::new(MockOutput::new());
    let mut temp_ctx = (*state.ctx).clone();
    temp_ctx.output = buffered_output.clone();

    let result = crate::commands::ApplyCommand::execute(
        &temp_ctx,
        req.path.as_deref(),
        false, // show_cost - not supported in UI yet
        false, // skip_policy - run validation in UI
        None,  // parallel - not supported in UI yet
        &req.executor_args,
    );

    let output_text = buffered_output.to_text();

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some(output_text),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: Some(output_text),
            error: Some(e.to_string()),
        }),
    }
}

async fn destroy(
    State(state): State<AppState>,
    Json(req): Json<DestroyRequest>,
) -> Json<ApiResponse<String>> {
    use crate::traits::output::MockOutput;
    use std::sync::Arc;

    let buffered_output = Arc::new(MockOutput::new());
    let mut temp_ctx = (*state.ctx).clone();
    temp_ctx.output = buffered_output.clone();

    let result = crate::commands::DestroyCommand::execute(
        &temp_ctx,
        req.path.as_deref(),
        req.yes,
        None, // parallel - not supported in UI yet
        &req.executor_args,
    );

    let output_text = buffered_output.to_text();

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some(output_text),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: Some(output_text),
            error: Some(e.to_string()),
        }),
    }
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<ExecutorRequest>,
) -> Json<ApiResponse<String>> {
    use crate::traits::output::MockOutput;
    use std::sync::Arc;

    let buffered_output = Arc::new(MockOutput::new());
    let mut temp_ctx = (*state.ctx).clone();
    temp_ctx.output = buffered_output.clone();

    let result = crate::commands::RefreshCommand::execute(
        &temp_ctx,
        req.path.as_deref(),
        &req.executor_args,
    );

    let output_text = buffered_output.to_text();

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            data: Some(output_text),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: Some(output_text),
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

async fn list_drives(State(state): State<AppState>) -> Json<ApiResponse<Vec<DriveInfo>>> {
    use std::path::PathBuf;

    let mut drives = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // On Windows, enumerate drive letters A: through Z:
        for letter in b'A'..=b'Z' {
            let drive_path = format!("{}:\\", letter as char);
            let path = PathBuf::from(&drive_path);

            // Check if the drive exists
            if state.ctx.fs.exists(&path) {
                drives.push(DriveInfo {
                    name: format!("{}: Drive", letter as char),
                    path: drive_path,
                });
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Unix-like systems, show root and common mount points
        let root = PathBuf::from("/");
        if state.ctx.fs.exists(&root) {
            drives.push(DriveInfo {
                name: "Root (/)".to_string(),
                path: "/".to_string(),
            });
        }

        // Check for common mount points
        let mount_points = vec![
            ("/mnt", "Mounts (/mnt)"),
            ("/media", "Media (/media)"),
            ("/home", "Home (/home)"),
        ];

        for (path_str, name) in mount_points {
            let path = PathBuf::from(path_str);
            if state.ctx.fs.exists(&path) && state.ctx.fs.is_dir(&path) {
                drives.push(DriveInfo {
                    name: name.to_string(),
                    path: path_str.to_string(),
                });
            }
        }
    }

    Json(ApiResponse {
        success: true,
        data: Some(drives),
        error: None,
    })
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

// ============================================================================
// WebSocket Handler for Streaming Operations
// ============================================================================

async fn ws_execute_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Wait for the operation request
    while let Some(msg) = receiver.next().await {
        if let Ok(Message::Text(text)) = msg {
            // Parse the operation request
            let request: Result<WsOperationRequest, _> = serde_json::from_str(&text);

            match request {
                Ok(req) => {
                    execute_streaming_operation(&mut sender, &state, req).await;
                }
                Err(e) => {
                    let error_msg = WsMessage {
                        msg_type: "error".to_string(),
                        data: serde_json::json!({ "message": format!("Invalid request: {}", e) }),
                    };
                    let _ = sender
                        .send(Message::Text(serde_json::to_string(&error_msg).unwrap()))
                        .await;
                }
            }
        }
    }
}

async fn execute_streaming_operation(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    req: WsOperationRequest,
) {
    use crate::traits::{StreamingOutput, format_output_message};

    let operation_id = uuid::Uuid::new_v4().to_string();
    let path = req.path.clone().unwrap_or_else(|| ".".to_string());

    // Send start message
    let start_msg = WsMessage {
        msg_type: "start".to_string(),
        data: serde_json::json!({
            "operation_id": operation_id,
            "operation": req.operation,
            "path": path
        }),
    };
    let _ = sender
        .send(Message::Text(serde_json::to_string(&start_msg).unwrap()))
        .await;

    // Track operation
    let now = chrono::Utc::now().to_rfc3339();
    {
        let mut ops = state.operations.lock().await;
        ops.insert(
            operation_id.clone(),
            OperationStatus {
                id: operation_id.clone(),
                operation: req.operation.clone(),
                project_path: path.clone(),
                status: "running".to_string(),
                started_at: now.clone(),
                finished_at: None,
                success: None,
            },
        );
    }

    // Create streaming output
    let (streaming_output, mut receiver) = StreamingOutput::new();
    let streaming_output = Arc::new(streaming_output);

    // Clone for the spawned task
    let ctx = (*state.ctx).clone();
    let mut temp_ctx = ctx;
    temp_ctx.output = streaming_output.clone();

    let operation = req.operation.clone();
    let executor_args = req.executor_args.unwrap_or_default();
    let yes = req.yes.unwrap_or(false);
    let path_clone = path.clone();

    // Spawn the operation in a separate task
    let handle = tokio::task::spawn_blocking(move || {
        match operation.as_str() {
            "preview" => crate::commands::PreviewCommand::execute(
                &temp_ctx,
                Some(&path_clone),
                false,  // show_cost - not supported in UI yet
                false,  // skip_policy - run validation in UI
                None,   // parallel - not supported in UI yet
                false,  // show_diff - not supported in UI yet
                "ascii", // diff_format
                false,  // side_by_side
                None,   // diff_output
                false,  // show_unchanged
                false,  // show_sensitive
                &executor_args,
            ),
            "apply" => crate::commands::ApplyCommand::execute(
                &temp_ctx,
                Some(&path_clone),
                false, // show_cost - not supported in UI yet
                false, // skip_policy - run validation in UI
                None,  // parallel - not supported in UI yet
                &executor_args,
            ),
            "destroy" => crate::commands::DestroyCommand::execute(
                &temp_ctx,
                Some(&path_clone),
                yes,
                None, // parallel - not supported in UI yet
                &executor_args,
            ),
            "refresh" => crate::commands::RefreshCommand::execute(
                &temp_ctx,
                Some(&path_clone),
                &executor_args,
            ),
            _ => Err(anyhow::anyhow!("Unknown operation: {}", operation)),
        }
    });

    // Pin the handle for use in the select loop
    let mut handle = std::pin::pin!(handle);
    let mut operation_complete = false;

    // Stream output messages as they arrive
    while !operation_complete {
        tokio::select! {
            msg = receiver.recv() => {
                match msg {
                    Ok(output_msg) => {
                        let text = format_output_message(&output_msg);
                        let ws_msg = WsMessage {
                            msg_type: "output".to_string(),
                            data: serde_json::json!({ "text": text }),
                        };

                        if sender.send(Message::Text(serde_json::to_string(&ws_msg).unwrap())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        operation_complete = true;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            result = &mut handle => {
                let success = matches!(result, Ok(Ok(_)));

                // Update operation status
                let finished_at = chrono::Utc::now().to_rfc3339();
                {
                    let mut ops = state.operations.lock().await;
                    if let Some(op) = ops.get_mut(&operation_id) {
                        op.status = if success { "completed".to_string() } else { "failed".to_string() };
                        op.finished_at = Some(finished_at.clone());
                        op.success = Some(success);
                    }
                }

                // Send completion message
                let end_msg = WsMessage {
                    msg_type: "complete".to_string(),
                    data: serde_json::json!({
                        "operation_id": operation_id,
                        "success": success,
                        "finished_at": finished_at
                    }),
                };
                let _ = sender.send(Message::Text(serde_json::to_string(&end_msg).unwrap())).await;
                operation_complete = true;
            }
        }
    }
}

// ============================================================================
// Dashboard API
// ============================================================================

#[derive(Debug, Serialize)]
struct DashboardData {
    infrastructure: Option<InfrastructureInfo>,
    project_count: usize,
    projects_by_kind: HashMap<String, usize>,
    projects_by_environment: HashMap<String, usize>,
    recent_operations: Vec<OperationStatus>,
    environments: Vec<String>,
}

async fn get_dashboard(State(state): State<AppState>) -> Json<ApiResponse<DashboardData>> {
    use crate::collection::CollectionDiscovery;

    // Load infrastructure
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
    let infrastructure = if state.ctx.fs.exists(&infra_path) {
        match crate::template::metadata::InfrastructureResource::from_file(
            &*state.ctx.fs,
            &infra_path,
        ) {
            Ok(infra) => {
                let categories: Vec<CategoryInfo> =
                    infra.spec.categories.iter().map(convert_category).collect();
                Some(InfrastructureInfo {
                    name: infra.metadata.name.clone(),
                    description: infra.metadata.description.clone(),
                    path: current_dir.to_string_lossy().to_string(),
                    environments: infra.spec.environments.keys().cloned().collect(),
                    categories,
                })
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Get environments list
    let environments: Vec<String> = infrastructure
        .as_ref()
        .map(|i| i.environments.clone())
        .unwrap_or_default();

    // Discover projects
    let projects = CollectionDiscovery::discover_projects(
        &*state.ctx.fs,
        &*state.ctx.output,
        &current_dir,
    )
    .unwrap_or_default();

    let project_count = projects.len();

    // Count projects by kind
    let mut projects_by_kind: HashMap<String, usize> = HashMap::new();
    for project in &projects {
        *projects_by_kind.entry(project.kind.clone()).or_insert(0) += 1;
    }

    // Count projects by environment
    let mut projects_by_environment: HashMap<String, usize> = HashMap::new();
    for project in &projects {
        let project_path = current_dir.join(&project.path);
        let envs = CollectionDiscovery::discover_environments(&*state.ctx.fs, &project_path)
            .unwrap_or_default();

        for env in envs {
            *projects_by_environment.entry(env).or_insert(0) += 1;
        }
    }

    // Get recent operations (last 10)
    let recent_operations: Vec<OperationStatus> = {
        let ops = state.operations.lock().await;
        let mut ops_vec: Vec<_> = ops.values().cloned().collect();
        ops_vec.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        ops_vec.into_iter().take(10).collect()
    };

    Json(ApiResponse {
        success: true,
        data: Some(DashboardData {
            infrastructure,
            project_count,
            projects_by_kind,
            projects_by_environment,
            recent_operations,
            environments,
        }),
        error: None,
    })
}

async fn list_operations(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<OperationStatus>>> {
    let ops = state.operations.lock().await;
    let mut ops_vec: Vec<_> = ops.values().cloned().collect();
    ops_vec.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    Json(ApiResponse {
        success: true,
        data: Some(ops_vec),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success_serialization() {
        let response: ApiResponse<String> = ApiResponse {
            success: true,
            data: Some("test data".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\":\"test data\""));
        assert!(json.contains("\"error\":null"));
    }

    #[test]
    fn test_api_response_error_serialization() {
        let response: ApiResponse<String> = ApiResponse {
            success: false,
            data: None,
            error: Some("Something went wrong".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"data\":null"));
        assert!(json.contains("\"error\":\"Something went wrong\""));
    }

    #[test]
    fn test_operation_status_serialization() {
        let status = OperationStatus {
            id: "op-123".to_string(),
            operation: "apply".to_string(),
            project_path: "/path/to/project".to_string(),
            status: "running".to_string(),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            finished_at: None,
            success: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"id\":\"op-123\""));
        assert!(json.contains("\"operation\":\"apply\""));
        assert!(json.contains("\"status\":\"running\""));
    }

    #[test]
    fn test_operation_status_completed_serialization() {
        let status = OperationStatus {
            id: "op-456".to_string(),
            operation: "destroy".to_string(),
            project_path: "/path/to/project".to_string(),
            status: "completed".to_string(),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            finished_at: Some("2024-01-01T00:05:00Z".to_string()),
            success: Some(true),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"finished_at\":\"2024-01-01T00:05:00Z\""));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_directory_entry_serialization() {
        let entry = DirectoryEntry {
            name: "src".to_string(),
            path: "/project/src".to_string(),
            is_dir: true,
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"name\":\"src\""));
        assert!(json.contains("\"is_dir\":true"));
    }

    #[test]
    fn test_drive_info_serialization() {
        let drive = DriveInfo {
            name: "C:".to_string(),
            path: "C:\\".to_string(),
        };

        let json = serde_json::to_string(&drive).unwrap();
        assert!(json.contains("\"name\":\"C:\""));
    }

    #[test]
    fn test_template_pack_info_serialization() {
        let pack = TemplatePackInfo {
            name: "aws-pack".to_string(),
            description: Some("AWS templates".to_string()),
            templates: vec![TemplateInfo {
                name: "vpc".to_string(),
                description: Some("VPC template".to_string()),
                kind: "VPC".to_string(),
                api_version: "pmp.io/v1".to_string(),
                inputs: vec![],
                environments: vec!["dev".to_string(), "prod".to_string()],
            }],
        };

        let json = serde_json::to_string(&pack).unwrap();
        assert!(json.contains("\"name\":\"aws-pack\""));
        assert!(json.contains("\"description\":\"AWS templates\""));
        assert!(json.contains("\"templates\":["));
    }

    #[test]
    fn test_input_info_string_type() {
        let input = InputInfo {
            name: "project_name".to_string(),
            input_type: "string".to_string(),
            description: Some("Name of the project".to_string()),
            default: Some(serde_json::json!("my-project")),
            required: false,
            options: None,
            min: None,
            max: None,
            conditions: None,
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"name\":\"project_name\""));
        assert!(json.contains("\"type\":\"string\""));
        assert!(json.contains("\"required\":false"));
    }

    #[test]
    fn test_input_info_number_type() {
        let input = InputInfo {
            name: "replicas".to_string(),
            input_type: "number".to_string(),
            description: Some("Number of replicas".to_string()),
            default: Some(serde_json::json!(3)),
            required: false,
            options: None,
            min: Some(1.0),
            max: Some(10.0),
            conditions: None,
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"number\""));
        assert!(json.contains("\"min\":1.0"));
        assert!(json.contains("\"max\":10.0"));
    }

    #[test]
    fn test_input_info_select_type() {
        let input = InputInfo {
            name: "environment".to_string(),
            input_type: "select".to_string(),
            description: Some("Environment".to_string()),
            default: Some(serde_json::json!("dev")),
            required: false,
            options: Some(vec![
                SelectOption {
                    label: "Development".to_string(),
                    value: "dev".to_string(),
                },
                SelectOption {
                    label: "Production".to_string(),
                    value: "prod".to_string(),
                },
            ]),
            min: None,
            max: None,
            conditions: None,
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"select\""));
        assert!(json.contains("\"options\":["));
        assert!(json.contains("\"label\":\"Development\""));
        assert!(json.contains("\"value\":\"dev\""));
    }

    #[test]
    fn test_input_info_with_conditions() {
        let input = InputInfo {
            name: "ssl_cert".to_string(),
            input_type: "string".to_string(),
            description: Some("SSL certificate path".to_string()),
            default: None,
            required: true,
            options: None,
            min: None,
            max: None,
            conditions: Some(vec![InputConditionInfo {
                field: "enable_ssl".to_string(),
                condition: "equals".to_string(),
                value: Some(serde_json::json!(true)),
            }]),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"conditions\":["));
        assert!(json.contains("\"field\":\"enable_ssl\""));
        assert!(json.contains("\"condition\":\"equals\""));
    }

    #[test]
    fn test_ws_message_serialization() {
        let msg = WsMessage {
            msg_type: "output".to_string(),
            data: serde_json::json!({"line": "Applying..."}),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"output\""));
        assert!(json.contains("\"data\":{"));
    }

    #[test]
    fn test_project_info_serialization() {
        let project = ProjectInfo {
            name: "my-vpc".to_string(),
            description: Some("Main VPC".to_string()),
            kind: "VPC".to_string(),
            path: "projects/my-vpc".to_string(),
            environments: vec!["dev".to_string(), "prod".to_string()],
        };

        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"name\":\"my-vpc\""));
        assert!(json.contains("\"kind\":\"VPC\""));
        assert!(json.contains("\"environments\":["));
    }

    #[test]
    fn test_dashboard_data_serialization() {
        let dashboard = DashboardData {
            infrastructure: None,
            project_count: 5,
            projects_by_kind: {
                let mut m = HashMap::new();
                m.insert("VPC".to_string(), 2);
                m.insert("Subnet".to_string(), 3);
                m
            },
            projects_by_environment: {
                let mut m = HashMap::new();
                m.insert("dev".to_string(), 5);
                m.insert("prod".to_string(), 3);
                m
            },
            recent_operations: vec![],
            environments: vec!["dev".to_string(), "prod".to_string()],
        };

        let json = serde_json::to_string(&dashboard).unwrap();
        assert!(json.contains("\"project_count\":5"));
        assert!(json.contains("\"projects_by_kind\":{"));
        assert!(json.contains("\"environments\":["));
    }

    #[test]
    fn test_find_request_deserialization() {
        let json = r#"{"name": "vpc", "kind": "VPC", "path": "/projects"}"#;
        let request: FindRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.name, Some("vpc".to_string()));
        assert_eq!(request.kind, Some("VPC".to_string()));
        assert_eq!(request.path, Some("/projects".to_string()));
    }

    #[test]
    fn test_find_request_partial_deserialization() {
        let json = r#"{"name": "vpc"}"#;
        let request: FindRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.name, Some("vpc".to_string()));
        assert_eq!(request.kind, None);
        assert_eq!(request.path, None);
    }

    #[test]
    fn test_executor_request_deserialization() {
        let json = r#"{"path": "/project/env", "executor_args": ["-auto-approve"]}"#;
        let request: ExecutorRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.path, Some("/project/env".to_string()));
        assert_eq!(request.executor_args, vec!["-auto-approve"]);
    }

    #[test]
    fn test_destroy_request_deserialization() {
        let json = r#"{"path": "/project/env", "yes": true, "executor_args": []}"#;
        let request: DestroyRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.path, Some("/project/env".to_string()));
        assert!(request.yes);
        assert!(request.executor_args.is_empty());
    }

    #[test]
    fn test_ws_operation_request_deserialization() {
        let json = r#"{"operation": "apply", "path": "/project", "executor_args": ["-target=aws_vpc.main"], "yes": true}"#;
        let request: WsOperationRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.operation, "apply");
        assert_eq!(request.path, Some("/project".to_string()));
        assert_eq!(request.yes, Some(true));
    }
}
