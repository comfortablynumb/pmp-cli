# Argo CD Template

This template installs [Argo CD](https://argo-cd.readthedocs.io/) on a Kubernetes cluster using Helm and OpenTofu.

## Description

Argo CD is a declarative, GitOps continuous delivery tool for Kubernetes. This template provides a configurable way to install Argo CD with support for:

- Custom namespace
- Configurable replicas for server and repo-server
- Optional ingress configuration
- High availability mode
- Custom admin password

## Prerequisites

- A Kubernetes cluster (minikube, kind, or cloud provider)
- kubectl configured with access to your cluster
- Helm repository access to https://argoproj.github.io/argo-helm

## Configuration Options

### Basic Settings

- **namespace**: Kubernetes namespace for Argo CD (default: `argocd`)
- **chart_version**: Argo CD Helm chart version (default: `5.51.6`)
- **create_namespace**: Whether to create the namespace if it doesn't exist (default: `true`)

### Server Configuration

- **server_replicas**: Number of Argo CD server replicas (default: `1`)
- **repo_server_replicas**: Number of Argo CD repo-server replicas (default: `1`)
- **admin_password**: Admin password (leave empty for auto-generated)

### Ingress Configuration

- **enable_ingress**: Enable ingress for Argo CD server (default: `false`)
- **server_host**: Hostname for Argo CD server (required if ingress is enabled)
- **ingress_class**: Ingress class name (default: `nginx`)

### High Availability

- **ha_enabled**: Enable high availability mode (default: `false`)
  - When enabled, sets up Redis HA, multiple controller replicas, and application set replicas

## Usage

1. Create a new project using this template:
   ```bash
   pmp create
   ```

2. Select the `argo-cd` template

3. Choose your environment and provide the configuration values

4. Preview the changes:
   ```bash
   cd projects/kubernetes_workload/<your-project>/environments/<environment>
   pmp preview
   ```

5. Apply the configuration:
   ```bash
   pmp apply
   ```

## Accessing Argo CD

### With Ingress Enabled

If you enabled ingress and configured a hostname, access Argo CD at:
```
https://<your-server-host>
```

### Without Ingress (Port Forward)

```bash
kubectl port-forward svc/argocd-server -n argocd 8080:443
```

Then access: https://localhost:8080

### Getting the Admin Password

If you didn't set a custom admin password, get the auto-generated one:

```bash
kubectl get secret argocd-initial-admin-secret -n argocd -o jsonpath="{.data.password}" | base64 -d
```

Default username: `admin`

## Example Configurations

### Minimal Installation

```
namespace: argocd
chart_version: 5.51.6
create_namespace: true
server_replicas: 1
repo_server_replicas: 1
ha_enabled: false
enable_ingress: false
```

### Production with HA and Ingress

```
namespace: argocd
chart_version: 5.51.6
create_namespace: true
server_replicas: 3
repo_server_replicas: 2
ha_enabled: true
enable_ingress: true
server_host: argocd.example.com
ingress_class: nginx
admin_password: <your-secure-password>
```

## Resources Created

- Kubernetes Namespace (if `create_namespace` is true)
- Helm Release for Argo CD
- Argo CD Server Deployment
- Argo CD Repo Server Deployment
- Argo CD Application Controller
- Argo CD Redis (or Redis HA if `ha_enabled`)
- Kubernetes Services
- Ingress (if `enable_ingress` is true)

## Post-Installation

After installation, you can:

1. Log in to the Argo CD UI
2. Configure Git repositories
3. Create applications
4. Set up SSO (optional)
5. Configure RBAC policies

For more information, see the [Argo CD documentation](https://argo-cd.readthedocs.io/).

## Troubleshooting

### Pods not starting

Check pod status:
```bash
kubectl get pods -n argocd
kubectl describe pod <pod-name> -n argocd
```

### Ingress not working

Verify ingress controller is installed:
```bash
kubectl get ingressclass
kubectl get ingress -n argocd
```

### Can't access UI

Check service status:
```bash
kubectl get svc -n argocd
kubectl logs -n argocd deployment/argocd-server
```

## Cleanup

To remove Argo CD:

```bash
pmp destroy  # (if this command exists)
# or manually:
helm uninstall argocd -n argocd
kubectl delete namespace argocd
```
