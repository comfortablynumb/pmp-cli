# Kubernetes Workloads Template Pack

This template pack provides production-ready Kubernetes workload templates for common deployment patterns.

## Templates

### http-api

Deploy an HTTP API service on Kubernetes with enterprise features:

- **Auto-scaling**: HorizontalPodAutoscaler (HPA) scales pods between min/max based on CPU utilization (80% threshold)
- **Health checks**: Startup and liveness probes ensure container health
- **Resource guarantees**: Guaranteed QoS with matching requests and limits
- **High availability**: PodDisruptionBudget ensures availability during disruptions
- **Database integration**: Support for PostgreSQL access plugin
- **Flexible service types**: ClusterIP, NodePort, or LoadBalancer

#### Resources Created

1. **Namespace**: Dedicated namespace for the application
2. **Deployment**: Manages pod replicas with health checks and resource limits
3. **Service**: Exposes HTTP port (configurable service type)
4. **HorizontalPodAutoscaler**: Auto-scales pods based on CPU usage
5. **PodDisruptionBudget**: Allows max 1 unavailable pod during disruptions

#### Configuration Inputs

| Input | Type | Default | Description |
|-------|------|---------|-------------|
| `docker_image` | string | (required) | Docker image name (e.g., nginx, myregistry.io/myapp) |
| `version` | string | (required) | Docker image tag/version (e.g., latest, v1.2.3) |
| `min_replicas` | number | 2 | Minimum number of pod replicas |
| `max_replicas` | number | 10 | Maximum number of pod replicas |
| `cpu_millicores` | enum | 250 | CPU limit in millicores (20-32000) |
| `memory_mb` | enum | 512 | Memory limit in MB (64-16384) |
| `http_port` | number | 8080 | Container HTTP port |
| `healthcheck_uri` | string | /_/health | HTTP path for health checks |
| `service_type` | enum | ClusterIP | Kubernetes service type |

#### CPU Options

20, 50, 100, 250, 500, 1000, 1500, 2000, 4000, 6000, 8000, 16000, 32000 millicores

#### Memory Options

64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384 MB

#### Service Types

- **ClusterIP**: Internal cluster access only (default)
- **NodePort**: Exposes service on each node's IP at a static port (useful for development)
- **LoadBalancer**: Exposes service externally using cloud provider's load balancer (production)

#### Plugin Support

This template supports the **PostgreSQL access plugin** from the `postgres` template pack. When you add this plugin, your API automatically receives database credentials as environment variables injected into the pods.

**Available environment variables from the access plugin**:
- `PLUGIN_POSTGRES_ACCESS_*_HOST`
- `PLUGIN_POSTGRES_ACCESS_*_PORT`
- `PLUGIN_POSTGRES_ACCESS_*_DATABASE`
- `PLUGIN_POSTGRES_ACCESS_*_USERNAME`
- `PLUGIN_POSTGRES_ACCESS_*_PASSWORD`

(The `*` is replaced with your plugin instance name in uppercase)

#### Health Check Configuration

- **Startup Probe**: Gives the container up to 60 seconds (12 failures × 5s) to start
- **Liveness Probe**: Checks every 10 seconds, restarts container after 3 consecutive failures

Your application must respond with HTTP 200 on the configured healthcheck URI.

#### Auto-scaling Behavior

- **Scale Up**: Aggressive - scales up by 100% or 2 pods (whichever is larger) every 30 seconds
- **Scale Down**: Conservative - scales down by 50% every 60 seconds after 5-minute stabilization
- **Target**: Maintains 80% average CPU utilization across all pods

#### Resource QoS

This template uses **Guaranteed QoS** by setting identical resource requests and limits. This ensures:
- Pods are never evicted due to resource pressure
- Predictable performance
- Priority in scheduling

## Usage Example

```bash
# Create a new HTTP API project
pmp create

# Select template pack: kubernetes-workloads
# Select template: http-api
# Select environment: development
# Provide inputs:
#   - Docker image: myregistry.io/my-api
#   - Version: v1.0.0
#   - Min replicas: 2
#   - Max replicas: 10
#   - CPU: 500m
#   - Memory: 1024Mi
#   - HTTP port: 8080
#   - Health check: /health
#   - Service type: ClusterIP

# Apply the infrastructure
cd projects/kubernetes_workload/my-api/environments/development
pmp apply

# View outputs
tofu output

# Access the service
kubectl port-forward -n my-api service/my-api 8080:8080
curl http://localhost:8080/health
```

## Prerequisites

- Kubernetes cluster (local or cloud)
- kubectl configured with access to the cluster
- OpenTofu/Terraform installed
- Docker registry with your application image

## Best Practices

1. **Production environments**: Use LoadBalancer service type with proper ingress controller
2. **Development environments**: Use NodePort or port-forwarding for local access
3. **Resource sizing**: Start conservative and scale up based on metrics
4. **Health checks**: Implement lightweight health check endpoints
5. **Image tags**: Use specific version tags instead of "latest" in production

## Monitoring

After deployment, monitor your application:

```bash
# Watch HPA status
kubectl get hpa -n <project-name> -w

# View pod metrics
kubectl top pods -n <project-name>

# Check pod events
kubectl describe deployment <project-name> -n <project-name>
```

## Troubleshooting

### Pods not starting

Check startup probe configuration and ensure your app starts within 60 seconds:

```bash
kubectl logs -n <project-name> -l app.kubernetes.io/name=<project-name>
```

### HPA not scaling

Ensure metrics-server is installed in your cluster:

```bash
kubectl get deployment metrics-server -n kube-system
```

### Service not accessible

Check service endpoints:

```bash
kubectl get endpoints <project-name> -n <project-name>
```

## License

This template pack is part of PMP (Poor Man's Platform).
