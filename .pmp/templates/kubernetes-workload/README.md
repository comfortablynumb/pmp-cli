# Kubernetes Workload Template

This template creates a complete Kubernetes workload with deployment, service, ingress, and autoscaling capabilities.

## Features

- **Deployment**: Configurable replicas and resource sizing
- **Monitoring**: Optional Prometheus annotations
- **Health Checks**: Optional liveness and readiness probes
- **Service**: Optional Kubernetes Service creation
- **Ingress**: Optional Ingress resource with hostname
- **Autoscaling**: Optional Horizontal Pod Autoscaler
- **Private Registry**: Optional registry credentials support

## Input Fields

### Required
- **name**: Project name (lowercase, alphanumeric with hyphens)
- **namespace**: Kubernetes namespace
- **replicas**: Number of replicas (1, 3, or 5)
- **resource_size**: CPU/Memory allocation (small, medium, large)
- **image**: Container image (format: image:tag)

### Optional
- **enable_monitoring**: Add Prometheus annotations (default: true)
- **features**: Multi-select for additional features
- **registry_password**: Password for private registry

## Generated Files

- `deployment.yaml`: Main Kubernetes Deployment
- `service.yaml`: Kubernetes Service (if selected)
- `ingress.yaml`: Ingress resource (if selected)
- `hpa.yaml`: Horizontal Pod Autoscaler (if selected)
- `secret.yaml`: Registry secret (if password provided)
- `.pmp.yaml`: Project configuration with kubectl hooks

## Example Usage

```bash
# Navigate to your ProjectCollection
cd my-infrastructure

# Create a new workload
pmp create

# Select this template and provide inputs
# The project will be created at: projects/kubernetes_workload/{project-name}/
```

## Resource Sizes

- **Small**: 0.5 CPU / 512Mi memory (limits: 1 CPU / 1Gi)
- **Medium**: 1 CPU / 1Gi memory (limits: 2 CPU / 2Gi)
- **Large**: 2 CPU / 2Gi memory (limits: 4 CPU / 4Gi)
