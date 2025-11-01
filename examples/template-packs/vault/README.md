# Vault Template

This template deploys HashiCorp Vault on Kubernetes using the official Helm chart. Vault is a tool for securely accessing secrets, encrypting data, and managing sensitive information.

## Features

- **Standalone or HA Mode**: Deploy Vault in standalone mode with file storage or HA mode with Raft integrated storage
- **Vault UI**: Built-in web interface for managing secrets
- **Vault Agent Injector**: Automatically inject secrets into Kubernetes pods via sidecar containers
- **Ingress Support**: Optional ingress configuration for external access
- **TLS Support**: Enable TLS encryption for secure communication
- **Resource Management**: Configure CPU and memory limits/requests
- **Persistent Storage**: Configurable persistent volume claims for data storage

## Configuration Options

### Core Settings

- **namespace**: Kubernetes namespace for Vault deployment (default: `vault`)
- **chart_version**: Vault Helm chart version (default: `0.28.1`)
- **create_namespace**: Whether to create the namespace if it doesn't exist (default: `true`)

### High Availability

- **ha_enabled**: Enable HA mode with Raft storage backend (default: `false`)
- **server_replicas**: Number of Vault server replicas (default: `1`, recommend 3 or 5 for HA)
- **storage_size**: Persistent volume size for Vault data (default: `10Gi`)

### UI Configuration

- **ui_enabled**: Enable the Vault web UI (default: `true`)
- **ui_service_type**: Service type for Vault UI - `ClusterIP`, `LoadBalancer`, or `NodePort` (default: `ClusterIP`)

### Ingress Configuration

- **enable_ingress**: Enable ingress for external access (default: `false`)
- **ingress_host**: Hostname for Vault ingress (e.g., `vault.example.com`)
- **ingress_class**: Ingress class name (default: `nginx`)
- **enable_tls**: Enable TLS for ingress (default: `false`)

### Vault Agent Injector

- **injector_enabled**: Enable Vault Agent Injector for sidecar injection (default: `true`)
- **injector_replicas**: Number of injector replicas (default: `1`)

### Resource Limits

- **resources_requests_cpu**: CPU request (default: `250m`)
- **resources_requests_memory**: Memory request (default: `256Mi`)
- **resources_limits_cpu**: CPU limit (default: `500m`)
- **resources_limits_memory**: Memory limit (default: `512Mi`)

## Deployment Examples

### Standalone Development Setup

```yaml
namespace: vault-dev
ha_enabled: false
server_replicas: 1
ui_enabled: true
ui_service_type: LoadBalancer
injector_enabled: true
storage_size: 5Gi
```

### Production HA Setup

```yaml
namespace: vault
ha_enabled: true
server_replicas: 3
ui_enabled: true
ui_service_type: ClusterIP
enable_ingress: true
ingress_host: vault.example.com
enable_tls: true
injector_enabled: true
storage_size: 20Gi
resources_requests_cpu: 500m
resources_requests_memory: 512Mi
resources_limits_cpu: 1000m
resources_limits_memory: 1Gi
```

## Post-Deployment Setup

After deploying Vault, you need to initialize and unseal it:

### 1. Access Vault

```bash
# Port forward to access Vault locally
kubectl port-forward -n vault svc/vault 8200:8200

# Or access via LoadBalancer/Ingress if configured
export VAULT_ADDR='http://localhost:8200'
```

### 2. Initialize Vault

```bash
# Initialize Vault (only do this once!)
vault operator init

# Save the unseal keys and root token securely!
# Example output:
# Unseal Key 1: abc123...
# Unseal Key 2: def456...
# ...
# Initial Root Token: s.xyz789...
```

### 3. Unseal Vault

Vault starts in a sealed state and requires unsealing with 3 out of 5 keys (by default):

```bash
vault operator unseal <unseal-key-1>
vault operator unseal <unseal-key-2>
vault operator unseal <unseal-key-3>
```

For HA deployments, unseal each replica:

```bash
kubectl exec -n vault vault-0 -- vault operator unseal <key>
kubectl exec -n vault vault-1 -- vault operator unseal <key>
kubectl exec -n vault vault-2 -- vault operator unseal <key>
```

### 4. Login

```bash
vault login <root-token>
```

## Using Vault Agent Injector

The Vault Agent Injector allows you to automatically inject secrets into your pods:

### 1. Configure Kubernetes Auth

```bash
# Enable Kubernetes auth
vault auth enable kubernetes

# Configure Kubernetes auth
vault write auth/kubernetes/config \
    kubernetes_host="https://$KUBERNETES_PORT_443_TCP_ADDR:443"

# Create a policy
vault policy write myapp-policy - <<EOF
path "secret/data/myapp/*" {
  capabilities = ["read"]
}
EOF

# Create a role
vault write auth/kubernetes/role/myapp \
    bound_service_account_names=myapp \
    bound_service_account_namespaces=default \
    policies=myapp-policy \
    ttl=24h
```

### 2. Add Annotations to Your Pods

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: myapp
  annotations:
    vault.hashicorp.com/agent-inject: "true"
    vault.hashicorp.com/role: "myapp"
    vault.hashicorp.com/agent-inject-secret-database: "secret/data/myapp/database"
    vault.hashicorp.com/agent-inject-template-database: |
      {{- with secret "secret/data/myapp/database" -}}
      export DB_USER="{{ .Data.data.username }}"
      export DB_PASS="{{ .Data.data.password }}"
      {{- end -}}
spec:
  serviceAccountName: myapp
  containers:
  - name: myapp
    image: myapp:latest
    command: ["/bin/sh", "-c"]
    args: ["source /vault/secrets/database && ./app"]
```

## Security Considerations

1. **Seal/Unseal Keys**: Store unseal keys securely (consider using Shamir's secret sharing)
2. **Root Token**: Revoke the initial root token after creating other auth methods
3. **Auto-Unseal**: For production, consider using cloud KMS for auto-unsealing
4. **TLS**: Always enable TLS in production environments
5. **Network Policies**: Restrict network access to Vault
6. **Audit Logging**: Enable audit logging for compliance

## Troubleshooting

### Check Vault Status

```bash
kubectl get pods -n vault
kubectl logs -n vault vault-0
vault status
```

### Common Issues

1. **Sealed Vault**: If pods are running but Vault is sealed, run unseal commands
2. **Storage Issues**: Check PVC status: `kubectl get pvc -n vault`
3. **HA Join Issues**: Check logs for Raft join errors
4. **Injector Not Working**: Verify service account and role bindings

## References

- [Vault Documentation](https://developer.hashicorp.com/vault/docs)
- [Vault Helm Chart](https://github.com/hashicorp/vault-helm)
- [Vault on Kubernetes Guide](https://developer.hashicorp.com/vault/tutorials/kubernetes)
- [Vault Agent Injector](https://developer.hashicorp.com/vault/docs/platform/k8s/injector)
