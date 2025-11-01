# Kubernetes Infrastructure Template Pack

This template pack provides production-ready infrastructure components for Kubernetes clusters, including service mesh and observability tools.

## Templates

### linkerd

Deploy Linkerd service mesh to provide secure, reliable, and observable communication between services in your Kubernetes cluster.

**Key Features:**
- **mTLS by default**: Automatic mutual TLS for all inter-service communication
- **Configurable CRD installation**: Choose whether to install CRDs or use existing ones
- **High availability mode**: Run control plane with multiple replicas for production
- **Flexible identity**: Support for Linkerd or Kubernetes certificate issuers
- **CNI plugin support**: Optional CNI installation for advanced networking
- **Resource control**: Configurable proxy sidecar resource limits
- **Observability**: Built-in distributed tracing support

#### Resources Created

1. **Namespace**: Dedicated namespace for Linkerd control plane (with proper labels)
2. **Linkerd CRDs** (optional): Custom Resource Definitions
3. **Linkerd Control Plane**: Core service mesh components via Helm

#### Configuration Inputs

| Input | Type | Default | Description |
|-------|------|---------|-------------|
| `namespace` | string | linkerd | Kubernetes namespace for control plane |
| `chart_version` | string | 1.16.11 | Linkerd Helm chart version |
| `create_namespace` | boolean | true | Create namespace if it doesn't exist |
| `install_crds` | boolean | true | Install Linkerd CRDs |
| `ha_enabled` | boolean | false | Enable high availability mode |
| `controller_replicas` | number | 1 | Number of controller replicas (1-5) |
| `identity_trust_domain` | string | cluster.local | Trust domain for mTLS |
| `identity_trust_anchors_pem` | string | "" | Trust anchors PEM (auto-generated if empty) |
| `identity_issuer_scheme` | enum | linkerd.io/tls | Certificate issuer scheme |
| `proxy_cpu_request` | string | 100m | Proxy CPU request |
| `proxy_cpu_limit` | string | 1000m | Proxy CPU limit |
| `proxy_memory_request` | string | 20Mi | Proxy memory request |
| `proxy_memory_limit` | string | 250Mi | Proxy memory limit |
| `cni_enabled` | boolean | false | Enable CNI plugin |
| `control_plane_tracing_enabled` | boolean | false | Enable control plane tracing |
| `proxy_await_before_exit_seconds` | number | 0 | Proxy graceful shutdown delay |
| `disable_heartbeat` | boolean | false | Disable heartbeat to Linkerd servers |
| `enable_external_profiles` | boolean | false | Enable external service profiles |

#### Usage Example

```bash
# Create Linkerd deployment
pmp create

# Select: kubernetes template pack
# Select: linkerd template
# Configure inputs as needed

# Apply
cd projects/linkerd/<project-name>/environments/<env>
pmp apply

# Verify installation
linkerd check

# Inject proxy into a deployment
kubectl get deploy -n myapp myapp-deployment -o yaml | linkerd inject - | kubectl apply -f -

# View dashboard
linkerd viz dashboard
```

#### Prerequisites

- Kubernetes cluster (1.21+)
- kubectl configured
- OpenTofu/Terraform installed
- linkerd CLI (for verification and viz)

---

### k8s-monitoring

Deploy Grafana's k8s-monitoring stack for comprehensive cluster observability with automatic integration detection for Grafana Cloud or custom endpoints.

**Key Features:**
- **Auto-detection**: Automatically configures for Grafana Cloud or custom endpoints
- **Metrics collection**: Cluster, node, and pod metrics via Prometheus
- **Logs aggregation**: Pod logs and cluster events via Loki
- **Distributed tracing**: Application traces via Tempo (optional)
- **Cost monitoring**: Optional OpenCost integration
- **Grafana Alloy**: Modern telemetry collector
- **Resource control**: Configurable resource limits for collection components

#### Resources Created

1. **Namespace**: Dedicated namespace for monitoring components
2. **Grafana Alloy**: Telemetry collector (DaemonSet/Deployment)
3. **Kube-State-Metrics** (optional): Cluster state metrics
4. **Node Exporter** (optional): Node-level metrics
5. **OpenCost** (optional): Cost monitoring

#### Configuration Inputs

| Input | Type | Default | Description |
|-------|------|---------|-------------|
| `namespace` | string | monitoring | Kubernetes namespace |
| `chart_version` | string | 1.5.7 | Chart version |
| `create_namespace` | boolean | true | Create namespace if needed |
| `cluster_name` | string | (required) | Cluster identifier |
| **Grafana Cloud** | | | |
| `grafana_cloud_url` | string | "" | Grafana Cloud URL (auto-detects if set) |
| `grafana_cloud_token` | string | "" | Grafana Cloud API token |
| **Custom Prometheus** | | | |
| `prometheus_url` | string | "" | Prometheus remote write URL |
| `prometheus_username` | string | "" | Prometheus auth username |
| `prometheus_password` | string | "" | Prometheus auth password |
| **Custom Loki** | | | |
| `loki_url` | string | "" | Loki URL |
| `loki_username` | string | "" | Loki auth username |
| `loki_password` | string | "" | Loki auth password |
| **Custom Tempo** | | | |
| `tempo_url` | string | "" | Tempo URL |
| `tempo_username` | string | "" | Tempo auth username |
| `tempo_password` | string | "" | Tempo auth password |
| **Feature Toggles** | | | |
| `metrics_enabled` | boolean | true | Enable metrics collection |
| `logs_enabled` | boolean | true | Enable logs collection |
| `traces_enabled` | boolean | false | Enable traces collection |
| `kube_state_metrics_enabled` | boolean | true | Enable kube-state-metrics |
| `node_exporter_enabled` | boolean | true | Enable node-exporter |
| `opencost_enabled` | boolean | false | Enable OpenCost |
| **Configuration** | | | |
| `log_level` | enum | info | Log level (debug/info/warn/error) |
| `prometheus_scrape_interval` | string | 60s | Scrape interval |
| `logs_pod_logs_enabled` | boolean | true | Collect pod logs |
| `logs_cluster_events_enabled` | boolean | true | Collect cluster events |
| **Resources** | | | |
| `alloy_cpu_request` | string | 200m | Alloy CPU request |
| `alloy_cpu_limit` | string | 1000m | Alloy CPU limit |
| `alloy_memory_request` | string | 128Mi | Alloy memory request |
| `alloy_memory_limit` | string | 512Mi | Alloy memory limit |

#### Auto-Detection Logic

The template automatically detects your monitoring backend:

**Grafana Cloud Mode** (if `grafana_cloud_url` is set):
- Uses Grafana Cloud URL for Prometheus, Loki, and Tempo
- Uses `grafana_cloud_token` for authentication
- Ignores custom endpoint configurations

**Custom Mode** (if `grafana_cloud_url` is empty):
- Uses individual endpoint URLs
- Uses individual authentication credentials
- Full control over each backend

#### Usage Examples

**Example 1: Grafana Cloud**

```bash
pmp create

# Inputs:
# - cluster_name: production-us-east-1
# - grafana_cloud_url: https://prometheus-prod-01-us-central-0.grafana.net
# - grafana_cloud_token: <your-token>
# - metrics_enabled: true
# - logs_enabled: true
# - traces_enabled: false

pmp apply
```

**Example 2: Self-Hosted**

```bash
pmp create

# Inputs:
# - cluster_name: staging-cluster
# - prometheus_url: https://prometheus.example.com/api/v1/push
# - prometheus_username: myuser
# - prometheus_password: mypass
# - loki_url: https://loki.example.com
# - loki_username: myuser
# - loki_password: mypass

pmp apply
```

**Example 3: Minimal (Metrics Only)**

```bash
pmp create

# Inputs:
# - cluster_name: dev-cluster
# - grafana_cloud_url: <grafana-cloud-url>
# - grafana_cloud_token: <token>
# - metrics_enabled: true
# - logs_enabled: false
# - traces_enabled: false

pmp apply
```

#### Verification

```bash
# Check deployment
kubectl get pods -n monitoring

# View Grafana Alloy logs
kubectl logs -n monitoring -l app.kubernetes.io/name=alloy -f

# Verify metrics are being sent
kubectl exec -n monitoring deploy/k8s-monitoring-alloy -- wget -O- localhost:12345/metrics

# In Grafana, query for your cluster
{cluster="your-cluster-name"}
```

#### Prerequisites

- Kubernetes cluster (1.23+)
- kubectl configured
- OpenTofu/Terraform installed
- Grafana Cloud account OR self-hosted Prometheus/Loki/Tempo

---

## Best Practices

### Linkerd

1. **Production deployments**: Enable HA mode with 3+ controller replicas
2. **Certificate management**: Use external cert-manager for long-lived certificates
3. **CNI plugin**: Enable for better network performance and security
4. **Proxy resources**: Adjust based on traffic volume and latency requirements
5. **Namespace injection**: Use `linkerd.io/inject: enabled` annotation for auto-injection

### K8s-Monitoring

1. **Resource sizing**: Start with defaults, scale based on cluster size
2. **Scrape intervals**: Balance between data freshness and resource usage
3. **Log filtering**: Use label filters to reduce log volume and costs
4. **Cost monitoring**: Enable OpenCost for production clusters
5. **High cardinality**: Be cautious with custom metrics and labels

## Integration Example

Deploy both Linkerd and k8s-monitoring together for a complete observability stack:

```bash
# 1. Deploy Linkerd
pmp create  # Select kubernetes > linkerd
cd projects/linkerd/my-mesh/environments/production
pmp apply

# 2. Deploy monitoring
pmp create  # Select kubernetes > k8s-monitoring
cd projects/k8s_monitoring/cluster-monitoring/environments/production
pmp apply

# 3. Enable Linkerd visualization
linkerd viz install | kubectl apply -f -
linkerd viz dashboard

# 4. View metrics in Grafana
# Use Linkerd-specific dashboards and queries
```

## Troubleshooting

### Linkerd

**Issue**: Pods fail to start with proxy injection errors
**Solution**: Check if CRDs are installed: `kubectl get crd | grep linkerd`

**Issue**: Certificate expiration warnings
**Solution**: Rotate certificates using `linkerd identity` commands

### K8s-Monitoring

**Issue**: No data in Grafana
**Solution**: Check Alloy logs for authentication errors or endpoint connectivity

**Issue**: High resource usage
**Solution**: Reduce scrape frequency or disable unnecessary collectors

**Issue**: Missing metrics
**Solution**: Verify kube-state-metrics and node-exporter are running

## License

This template pack is part of PMP (Poor Man's Platform).
