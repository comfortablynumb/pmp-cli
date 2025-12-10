//! Test helpers for creating sample template packs and infrastructure setups
//!
//! This module provides utilities for setting up comprehensive test environments
//! with template packs, plugins, and infrastructure configurations.

#![cfg(test)]

use crate::traits::{FileSystem, MockFileSystem};
use std::path::PathBuf;

/// Builder for creating a comprehensive template pack for testing
pub struct TemplatePackBuilder {
    pack_name: String,
    pack_description: String,
    templates: Vec<TemplateBuilder>,
    plugins: Vec<PluginBuilder>,
}

/// Builder for creating a template
pub struct TemplateBuilder {
    name: String,
    description: String,
    api_version: String,
    kind: String,
    executor: String,
    order: i32,
    inputs: String,
    dependencies: String,
    environments: String,
    installed_plugins: Vec<InstalledPluginConfig>,
    allowed_plugins: Vec<AllowedPluginConfig>,
    template_files: Vec<(String, String)>, // (path, content)
}

/// Builder for creating a plugin
pub struct PluginBuilder {
    name: String,
    description: String,
    role: String,
    inputs: String,
    dependencies: String,
    plugin_files: Vec<(String, String)>, // (path, content)
}

/// Configuration for an installed plugin
pub struct InstalledPluginConfig {
    template_pack_name: String,
    plugin_name: String,
    order: i32,
    disable_user_input_override: bool,
    inputs: Option<String>,
}

/// Configuration for an allowed plugin
pub struct AllowedPluginConfig {
    template_pack_name: String,
    plugin_name: String,
}

impl TemplatePackBuilder {
    /// Create a new template pack builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            pack_name: name.into(),
            pack_description: "Test template pack".to_string(),
            templates: Vec::new(),
            plugins: Vec::new(),
        }
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.pack_description = desc.into();
        self
    }

    /// Add a template
    pub fn template(mut self, template: TemplateBuilder) -> Self {
        self.templates.push(template);
        self
    }

    /// Add a plugin
    pub fn plugin(mut self, plugin: PluginBuilder) -> Self {
        self.plugins.push(plugin);
        self
    }

    /// Build the template pack in the mock filesystem and return the path
    pub fn build(self, fs: &MockFileSystem, base_path: PathBuf) -> PathBuf {
        let pack_path = base_path.join(&self.pack_name);

        // Create template pack file
        let pack_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: {}
  description: {}
spec: {{}}"#,
            self.pack_name, self.pack_description
        );
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), &pack_yaml)
            .unwrap();

        // Create templates
        for template in self.templates {
            template.build(fs, pack_path.join("templates"));
        }

        // Create plugins
        for plugin in self.plugins {
            plugin.build(fs, pack_path.join("plugins"));
        }

        pack_path
    }
}

impl TemplateBuilder {
    /// Create a new template builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: "Test template".to_string(),
            api_version: "pmp.io/v1".to_string(),
            kind: "TestResource".to_string(),
            executor: "opentofu".to_string(),
            order: 0,
            inputs: String::new(),
            dependencies: String::new(),
            environments: String::new(),
            installed_plugins: Vec::new(),
            allowed_plugins: Vec::new(),
            template_files: Vec::new(),
        }
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the API version and kind
    pub fn resource(mut self, api_version: impl Into<String>, kind: impl Into<String>) -> Self {
        self.api_version = api_version.into();
        self.kind = kind.into();
        self
    }

    /// Set the executor
    pub fn executor(mut self, executor: impl Into<String>) -> Self {
        self.executor = executor.into();
        self
    }

    /// Set the order
    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Set inputs YAML (indented)
    pub fn inputs(mut self, inputs: impl Into<String>) -> Self {
        self.inputs = inputs.into();
        self
    }

    /// Set dependencies YAML (indented)
    pub fn dependencies(mut self, deps: impl Into<String>) -> Self {
        self.dependencies = deps.into();
        self
    }

    /// Set environments YAML (indented)
    pub fn environments(mut self, envs: impl Into<String>) -> Self {
        self.environments = envs.into();
        self
    }

    /// Add an installed plugin
    pub fn with_installed_plugin(mut self, config: InstalledPluginConfig) -> Self {
        self.installed_plugins.push(config);
        self
    }

    /// Add an allowed plugin
    pub fn with_allowed_plugin(mut self, config: AllowedPluginConfig) -> Self {
        self.allowed_plugins.push(config);
        self
    }

    /// Add a template file
    pub fn with_file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.template_files.push((path.into(), content.into()));
        self
    }

    /// Build the template in the mock filesystem
    fn build(self, fs: &MockFileSystem, base_path: PathBuf) {
        let template_dir = base_path.join(&self.name);

        // Build plugins section
        let mut plugins_section = String::new();
        if !self.installed_plugins.is_empty() || !self.allowed_plugins.is_empty() {
            plugins_section.push_str("  plugins:\n");

            if !self.installed_plugins.is_empty() {
                plugins_section.push_str("    installed:\n");
                for plugin in &self.installed_plugins {
                    plugins_section.push_str(&format!(
                        "      - template_pack_name: {}\n",
                        plugin.template_pack_name
                    ));
                    plugins_section.push_str(&format!("        plugin_name: {}\n", plugin.plugin_name));
                    plugins_section.push_str(&format!("        order: {}\n", plugin.order));
                    if plugin.disable_user_input_override {
                        plugins_section.push_str("        disable_user_input_override: true\n");
                    }
                    if let Some(inputs) = &plugin.inputs {
                        plugins_section.push_str("        inputs:\n");
                        for line in inputs.lines() {
                            plugins_section.push_str(&format!("          {}\n", line));
                        }
                    }
                }
            }

            if !self.allowed_plugins.is_empty() {
                plugins_section.push_str("    allowed:\n");
                for plugin in &self.allowed_plugins {
                    plugins_section.push_str(&format!(
                        "      - template_pack_name: {}\n",
                        plugin.template_pack_name
                    ));
                    plugins_section.push_str(&format!("        plugin_name: {}\n", plugin.plugin_name));
                }
            }
        }

        // Build dependencies section
        let deps_section = if !self.dependencies.is_empty() {
            format!("  dependencies:\n{}\n", self.dependencies)
        } else {
            String::new()
        };

        // Build environments section
        let envs_section = if !self.environments.is_empty() {
            format!("  environments:\n{}\n", self.environments)
        } else {
            String::new()
        };

        // Create template file
        let template_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: {}
  description: {}
spec:
  apiVersion: {}
  kind: {}
  executor: {}
  order: {}
{}{}{}{}  inputs:
{}"#,
            self.name,
            self.description,
            self.api_version,
            self.kind,
            self.executor,
            self.order,
            deps_section,
            envs_section,
            plugins_section,
            if self.inputs.is_empty() { "    {}" } else { "" },
            self.inputs
        );
        fs.write(&template_dir.join(".pmp.template.yaml"), &template_yaml)
            .unwrap();

        // Create template files
        if self.template_files.is_empty() {
            // Create a default template file
            fs.write(&template_dir.join("src/main.tf.hbs"), "# Test template")
                .unwrap();
        } else {
            for (path, content) in &self.template_files {
                fs.write(&template_dir.join("src").join(path), content)
                    .unwrap();
            }
        }
    }
}

impl PluginBuilder {
    /// Create a new plugin builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: "Test plugin".to_string(),
            role: "default".to_string(),
            inputs: String::new(),
            dependencies: String::new(),
            plugin_files: Vec::new(),
        }
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the role
    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.role = role.into();
        self
    }

    /// Set inputs YAML (indented)
    pub fn inputs(mut self, inputs: impl Into<String>) -> Self {
        self.inputs = inputs.into();
        self
    }

    /// Set dependencies YAML (indented)
    pub fn dependencies(mut self, deps: impl Into<String>) -> Self {
        self.dependencies = deps.into();
        self
    }

    /// Add a plugin file
    pub fn with_file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.plugin_files.push((path.into(), content.into()));
        self
    }

    /// Build the plugin in the mock filesystem
    fn build(self, fs: &MockFileSystem, base_path: PathBuf) {
        let plugin_dir = base_path.join(&self.name);

        // Build dependencies section
        let deps_section = if !self.dependencies.is_empty() {
            format!("  dependencies:\n{}\n", self.dependencies)
        } else {
            String::new()
        };

        // Create plugin file
        let plugin_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Plugin
metadata:
  name: {}
  description: {}
spec:
  role: {}
{}  inputs:
{}"#,
            self.name,
            self.description,
            self.role,
            deps_section,
            if self.inputs.is_empty() { "    {}" } else { &self.inputs }
        );
        fs.write(&plugin_dir.join(".pmp.plugin.yaml"), &plugin_yaml)
            .unwrap();

        // Create plugin files
        if self.plugin_files.is_empty() {
            // Create a default plugin file
            fs.write(&plugin_dir.join("src/plugin.tf.hbs"), "# Test plugin")
                .unwrap();
        } else {
            for (path, content) in &self.plugin_files {
                fs.write(&plugin_dir.join("src").join(path), content)
                    .unwrap();
            }
        }
    }
}

/// Create a comprehensive sample template pack with all available features
///
/// This creates a template pack with:
/// - Multiple input types (string, number, boolean, select, project_reference, projects_reference)
/// - Conditional inputs
/// - Environment-specific inputs
/// - Dependencies
/// - Installed plugins
/// - Allowed plugins
pub fn create_comprehensive_template_pack(fs: &MockFileSystem) -> PathBuf {
    let current_dir = fs.current_dir().unwrap();
    let base_path = current_dir.join(".pmp/template-packs");

    TemplatePackBuilder::new("comprehensive-pack")
        .description("Comprehensive test template pack with all features")
        // Template with all input types
        .template(
            TemplateBuilder::new("full-featured-template")
                .description("Template demonstrating all input types")
                .resource("pmp.io/v1", "Application")
                .executor("opentofu")
                .order(100)
                .inputs(r#"    # String input
    app_name:
      type: string
      description: Application name
      default: my-app

    # Number input with constraints
    replica_count:
      type: number
      description: Number of replicas
      default: 3
      min: 1
      max: 10

    # Boolean input
    enable_monitoring:
      type: boolean
      description: Enable monitoring
      default: true

    # Select input
    deployment_strategy:
      type: select
      description: Deployment strategy
      default: rolling
      options:
        - rolling
        - blue-green
        - canary

    # Project reference (single)
    database_project:
      type: project_reference
      description: Database project to connect to
      api_version: pmp.io/v1
      kind: Database
      label_selector:
        tier: data

    # Projects reference (multiple)
    cache_projects:
      type: projects_reference
      description: Cache projects
      api_version: pmp.io/v1
      kind: Cache
      min: 1
      max: 3

    # Conditional input
    monitoring_endpoint:
      type: string
      description: Monitoring endpoint URL
      show_if:
        - field: enable_monitoring
          equals: true

    # Environment-specific input
    log_level:
      type: select
      description: Log level
      default: info
      options:
        - debug
        - info
        - warn
        - error
      environments:
        dev:
          default: debug
        prod:
          default: warn"#)
                .dependencies(r#"    - dependency_name: main_database
      project:
        apiVersion: pmp.io/v1
        kind: Database
        label_selector:
          tier: primary
        description: Main database for the application
        remote_state:
          data_source_name: main_db"#)
                .environments(r#"    - dev
    - staging
    - prod"#)
                .with_installed_plugin(InstalledPluginConfig {
                    template_pack_name: "comprehensive-pack".to_string(),
                    plugin_name: "monitoring-plugin".to_string(),
                    order: 50,
                    disable_user_input_override: false,
                    inputs: Some("prometheus_enabled:\n  value: true".to_string()),
                })
                .with_installed_plugin(InstalledPluginConfig {
                    template_pack_name: "comprehensive-pack".to_string(),
                    plugin_name: "backup-plugin".to_string(),
                    order: 200,
                    disable_user_input_override: true,
                    inputs: Some("backup_schedule:\n  value: \"0 2 * * *\"".to_string()),
                })
                .with_allowed_plugin(AllowedPluginConfig {
                    template_pack_name: "comprehensive-pack".to_string(),
                    plugin_name: "logging-plugin".to_string(),
                })
                .with_file("main.tf.hbs", r#"# Application: {{app_name}}
# Replicas: {{replica_count}}
# Monitoring: {{enable_monitoring}}
# Strategy: {{deployment_strategy}}

resource "kubernetes_deployment" "app" {
  metadata {
    name = "{{app_name}}"
  }

  spec {
    replicas = {{replica_count}}

    {{#if enable_monitoring}}
    # Monitoring enabled
    {{/if}}
  }
}
"#)
        )
        // Simple template for basic tests
        .template(
            TemplateBuilder::new("simple-template")
                .description("Simple template for basic tests")
                .resource("pmp.io/v1", "SimpleResource")
                .executor("opentofu")
                .inputs(r#"    name:
      type: string
      description: Resource name
      default: simple"#)
                .with_file("simple.tf.hbs", "# Simple resource: {{name}}")
        )
        // Plugin with dependencies
        .plugin(
            PluginBuilder::new("monitoring-plugin")
                .description("Monitoring plugin with Prometheus")
                .role("observability")
                .inputs(r#"    prometheus_enabled:
      type: boolean
      description: Enable Prometheus
      default: true

    grafana_enabled:
      type: boolean
      description: Enable Grafana
      default: false

    retention_days:
      type: number
      description: Metrics retention in days
      default: 30
      min: 7
      max: 365"#)
                .with_file("monitoring.tf.hbs", r#"# Monitoring configuration
{{#if prometheus_enabled}}
resource "helm_release" "prometheus" {
  name = "prometheus"
  # ... configuration
}
{{/if}}

{{#if grafana_enabled}}
resource "helm_release" "grafana" {
  name = "grafana"
  # ... configuration
}
{{/if}}
"#)
        )
        // Plugin with project dependencies
        .plugin(
            PluginBuilder::new("backup-plugin")
                .description("Backup plugin with storage dependency")
                .role("data-protection")
                .inputs(r#"    backup_schedule:
      type: string
      description: Cron schedule for backups
      default: "0 2 * * *"

    retention_count:
      type: number
      description: Number of backups to retain
      default: 7"#)
                .dependencies(r#"    - dependency_name: storage
      project:
        apiVersion: pmp.io/v1
        kind: ObjectStorage
        description: Storage for backups
        remote_state:
          data_source_name: backup_storage"#)
                .with_file("backup.tf.hbs", r#"# Backup configuration
# Schedule: {{backup_schedule}}
# Retention: {{retention_count}}

data "terraform_remote_state" "backup_storage" {
  # Storage reference
}

resource "kubernetes_cron_job" "backup" {
  schedule = "{{backup_schedule}}"
  # ... configuration
}
"#)
        )
        // Simple plugin for allowed plugins
        .plugin(
            PluginBuilder::new("logging-plugin")
                .description("Logging aggregation plugin")
                .role("observability")
                .inputs(r#"    log_aggregator:
      type: select
      description: Log aggregation service
      default: loki
      options:
        - loki
        - elasticsearch
        - cloudwatch"#)
                .with_file("logging.tf.hbs", "# Logging: {{log_aggregator}}")
        )
        .build(fs, base_path)
}

/// Create an OpenTofu template pack for testing project create/update workflows
///
/// This creates a template pack with:
/// - OpenTofu executor with S3 backend configuration
/// - A web application template with standard inputs
/// - Pre-installed monitoring plugin
/// - Available logging and backup plugins
pub fn create_opentofu_template_pack(fs: &MockFileSystem) -> PathBuf {
    let current_dir = fs.current_dir().unwrap();
    let base_path = current_dir.join(".pmp/template-packs");

    TemplatePackBuilder::new("opentofu-pack")
        .description("OpenTofu template pack for testing")
        // Web application template with pre-installed monitoring
        .template(
            TemplateBuilder::new("webapp")
                .description("Web application with monitoring")
                .resource("pmp.io/v1", "WebApp")
                .executor("opentofu")
                .order(100)
                .inputs(r#"    app_name:
      type: string
      description: Application name
      default: my-app

    port:
      type: number
      description: Application port
      default: 8080
      min: 1024
      max: 65535

    enable_tls:
      type: boolean
      description: Enable TLS
      default: true

    environment_type:
      type: select
      description: Environment type
      default: development
      options:
        - label: "Development"
          value: "development"
        - label: "Staging"
          value: "staging"
        - label: "Production"
          value: "production""#)
                .with_installed_plugin(InstalledPluginConfig {
                    template_pack_name: "opentofu-pack".to_string(),
                    plugin_name: "monitoring".to_string(),
                    order: 50,
                    disable_user_input_override: false,
                    inputs: None, // Use plugin defaults
                })
                .with_allowed_plugin(AllowedPluginConfig {
                    template_pack_name: "opentofu-pack".to_string(),
                    plugin_name: "logging".to_string(),
                })
                .with_allowed_plugin(AllowedPluginConfig {
                    template_pack_name: "opentofu-pack".to_string(),
                    plugin_name: "backup".to_string(),
                })
                .with_file("main.tf.hbs", r#"# Web Application: {{app_name}}
# Port: {{port}}
# TLS: {{enable_tls}}
# Environment: {{environment_type}}

resource "kubernetes_deployment" "webapp" {
  metadata {
    name = "{{app_name}}"
  }

  spec {
    replicas = 2

    selector {
      match_labels = {
        app = "{{app_name}}"
      }
    }

    template {
      metadata {
        labels = {
          app = "{{app_name}}"
        }
      }

      spec {
        container {
          name  = "{{app_name}}"
          image = "nginx:latest"

          port {
            container_port = {{port}}
          }

          {{#if enable_tls}}
          # TLS configuration
          {{/if}}
        }
      }
    }
  }
}

resource "kubernetes_service" "webapp" {
  metadata {
    name = "{{app_name}}-svc"
  }

  spec {
    selector = {
      app = "{{app_name}}"
    }

    port {
      port        = {{port}}
      target_port = {{port}}
    }

    type = "{{#if (eq environment_type "production")}}LoadBalancer{{else}}ClusterIP{{/if}}"
  }
}
"#)
        )
        // Monitoring plugin (pre-installed)
        .plugin(
            PluginBuilder::new("monitoring")
                .description("Prometheus monitoring")
                .role("observability")
                .inputs(r#"    metrics_enabled:
      type: boolean
      description: Enable metrics collection
      default: true

    scrape_interval:
      type: string
      description: Metrics scrape interval
      default: "30s""#)
                .with_file("monitoring.tf.hbs", r#"# Monitoring configuration
# Metrics: {{metrics_enabled}}
# Scrape interval: {{scrape_interval}}

{{#if metrics_enabled}}
resource "kubernetes_config_map" "prometheus" {
  metadata {
    name = "prometheus-config"
  }

  data = {
    "prometheus.yml" = <<-EOT
      global:
        scrape_interval: {{scrape_interval}}

      scrape_configs:
        - job_name: 'webapp'
          static_configs:
            - targets: ['localhost:9090']
    EOT
  }
}

resource "kubernetes_deployment" "prometheus" {
  metadata {
    name = "prometheus"
  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "prometheus"
      }
    }

    template {
      metadata {
        labels = {
          app = "prometheus"
        }
      }

      spec {
        container {
          name  = "prometheus"
          image = "prom/prometheus:latest"
        }
      }
    }
  }
}
{{/if}}
"#)
        )
        // Logging plugin (allowed, not pre-installed)
        .plugin(
            PluginBuilder::new("logging")
                .description("Centralized logging")
                .role("observability")
                .inputs(r#"    log_level:
      type: select
      description: Log level
      default: info
      options:
        - debug
        - info
        - warn
        - error

    retention_days:
      type: number
      description: Log retention in days
      default: 30
      min: 1
      max: 365"#)
                .with_file("logging.tf.hbs", r#"# Logging configuration
# Log level: {{log_level}}
# Retention: {{retention_days}} days

resource "kubernetes_config_map" "fluentd" {
  metadata {
    name = "fluentd-config"
  }

  data = {
    "fluent.conf" = <<-EOT
      <source>
        @type tail
        path /var/log/containers/*.log
        tag kubernetes.*
      </source>

      <filter kubernetes.**>
        @type record_transformer
        <record>
          log_level {{log_level}}
        </record>
      </filter>

      <match kubernetes.**>
        @type elasticsearch
        logstash_format true
        logstash_prefix kubernetes
      </match>
    EOT
  }
}

resource "kubernetes_daemonset" "fluentd" {
  metadata {
    name = "fluentd"
  }

  spec {
    selector {
      match_labels = {
        app = "fluentd"
      }
    }

    template {
      metadata {
        labels = {
          app = "fluentd"
        }
      }

      spec {
        container {
          name  = "fluentd"
          image = "fluent/fluentd:latest"
        }
      }
    }
  }
}
"#)
        )
        // Backup plugin (allowed, not pre-installed)
        .plugin(
            PluginBuilder::new("backup")
                .description("Automated backups")
                .role("data-protection")
                .inputs(r#"    backup_schedule:
      type: string
      description: Cron schedule for backups
      default: "0 2 * * *"

    backup_retention:
      type: number
      description: Number of backups to retain
      default: 7
      min: 1
      max: 30"#)
                .with_file("backup.tf.hbs", r#"# Backup configuration
# Schedule: {{backup_schedule}}
# Retention: {{backup_retention}} backups

resource "kubernetes_cron_job" "backup" {
  metadata {
    name = "backup-job"
  }

  spec {
    schedule = "{{backup_schedule}}"

    job_template {
      metadata {
        name = "backup"
      }

      spec {
        template {
          metadata {
            labels = {
              app = "backup"
            }
          }

          spec {
            container {
              name    = "backup"
              image   = "backup:latest"
              command = ["/backup.sh"]

              env {
                name  = "RETENTION"
                value = "{{backup_retention}}"
              }
            }

            restart_policy = "OnFailure"
          }
        }
      }
    }
  }
}
"#)
        )
        .build(fs, base_path)
}

/// Create infrastructure for OpenTofu template pack testing
pub fn create_opentofu_infrastructure(fs: &MockFileSystem, environments: &[&str]) {
    let current_dir = fs.current_dir().unwrap();
    let infra_file = current_dir.join(".pmp.infrastructure.yaml");

    let envs_yaml = environments
        .iter()
        .map(|e| {
            format!(
                "    {}:\n      name: {}\n      description: {} environment",
                e,
                e.chars().next().unwrap().to_uppercase().to_string() + &e[1..],
                e.chars().next().unwrap().to_uppercase().to_string() + &e[1..]
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let infra_yaml = format!(
        r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: test-infrastructure
  description: Test infrastructure with OpenTofu
spec:
  environments:
{}
  categories:
    - id: webapps
      name: Web Applications
      description: Web application templates
      templates:
        - template_pack: opentofu-pack
          template: webapp
  executor:
    name: opentofu
    config:
      backend:
        type: s3
        bucket: my-terraform-state
        key: "{{{{project_name}}}}/{{{{environment}}}}/terraform.tfstate"
        region: us-east-1
        encrypt: true
"#,
        envs_yaml
    );

    fs.write(&infra_file, &infra_yaml).unwrap();

    // Create projects directory so infrastructure is recognized
    fs.create_dir_all(&current_dir.join("projects")).unwrap();
}

/// Create a simple infrastructure configuration for testing
pub fn create_test_infrastructure(fs: &MockFileSystem, environments: &[&str]) {
    let current_dir = fs.current_dir().unwrap();
    let infra_file = current_dir.join(".pmp.infrastructure.yaml");

    let envs_yaml = environments
        .iter()
        .map(|e| {
            format!(
                "    {}:\n      name: {}\n      description: {} environment",
                e,
                e.chars().next().unwrap().to_uppercase().to_string() + &e[1..],
                e.chars().next().unwrap().to_uppercase().to_string() + &e[1..]
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let infra_yaml = format!(
        r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: test-infrastructure
  description: Test infrastructure
spec:
  environments:
{}
  categories:
    - id: applications
      name: Applications
      description: Application templates
      templates:
        - template_pack: comprehensive-pack
          template: full-featured-template
        - template_pack: comprehensive-pack
          template: simple-template
"#,
        envs_yaml
    );

    fs.write(&infra_file, &infra_yaml).unwrap();

    // Create projects directory so infrastructure is recognized
    fs.create_dir_all(&current_dir.join("projects")).unwrap();
}
