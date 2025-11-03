# Minikube HTTP Echo Template

This template creates a Kubernetes workload for local minikube using the `hashicorp/http-echo` image, deployed via OpenTofu/Terraform.

## What it creates

- **OpenTofu/Terraform Configuration**: Infrastructure as Code for Kubernetes resources
- **Kubernetes Deployment**: Running the hashicorp/http-echo container
- **Kubernetes Service**: Exposing the deployment

## Generated Files

- `main.tf`: Main OpenTofu configuration with deployment and service resources
- `variables.tf`: Variable definitions with defaults from template inputs
- `outputs.tf`: Output values for deployed resources

## Features

- Configurable number of replicas
- Configurable port (default: 8080)
- Customizable echo message
- Configurable service type (ClusterIP, NodePort, or LoadBalancer)
- Resource requests and limits pre-configured for local development
- Uses Kubernetes provider for OpenTofu/Terraform

## Template Inputs

- `replicas` (default: 1): Number of pod replicas
- `port` (default: 8080): Container port for the http-echo service
- `text` (default: "Hello from minikube!"): Text message to be echoed by the service
- `service_type` (default: NodePort): Kubernetes service type (ClusterIP, NodePort, or LoadBalancer)

## Prerequisites

1. **Minikube** installed and running:
   ```bash
   minikube start
   ```

2. **OpenTofu or Terraform** installed:
   ```bash
   # OpenTofu
   brew install opentofu

   # Or Terraform
   brew install terraform
   ```

3. **kubectl** configured to use minikube context:
   ```bash
   kubectl config use-context minikube
   ```

## Usage with PMP

1. Ensure you have a `.pmp.infrastructure.yaml` that allows `KubernetesWorkload` kind:
   ```yaml
   spec:
     resource_kinds:
       - apiVersion: pmp.io/v1
         kind: KubernetesWorkload
     environments:
       dev:
         name: Development
   ```

2. Run `pmp create` (or `cargo run -- create` for development)
3. Select the `minikube-http-echo` template
4. Provide the required inputs (name, environment, custom inputs)
5. The project will be generated in `projects/kubernetes_workload/{project-name}/`

## Deploying to Minikube

After creating a project from this template:

```bash
# Navigate to the project directory
cd projects/kubernetes_workload/{your-project-name}

# Initialize OpenTofu/Terraform
tofu init
# or: terraform init

# Preview the changes
tofu plan
# or: terraform plan

# Apply the infrastructure
tofu apply
# or: terraform apply

# View outputs
tofu output
# or: terraform output
```

## Accessing the Service

If you're using NodePort (default):

```bash
# Get the service URL
minikube service {your-project-name} --url

# Test with curl
curl $(minikube service {your-project-name} --url)

# Or open in browser
minikube service {your-project-name}
```

Expected output:
```
Hello from minikube!
```

## Destroying Resources

When you're done:

```bash
tofu destroy
# or: terraform destroy
```

## Example Project

```yaml
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: my-echo-api
  description: Example HTTP echo service
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: KubernetesWorkload
  iac:
    executor: opentofu
  inputs:
    replicas: 2
    port: 8080
    text: "Welcome to my API!"
    service_type: NodePort
```

## Customizing the Deployment

You can override variables when applying:

```bash
# Override replicas
tofu apply -var="replicas=3"

# Override the echo text
tofu apply -var='text=Custom message'

# Use a different service type
tofu apply -var="service_type=LoadBalancer"
```

## Troubleshooting

### Provider Configuration Issues

If you get authentication errors, ensure your kubectl context is set correctly:

```bash
kubectl config current-context  # Should show "minikube"
kubectl config use-context minikube
```

### NodePort Not Accessible

If using minikube on certain platforms, you may need to run:

```bash
minikube tunnel
```

Or access via the minikube service command:

```bash
minikube service {your-project-name}
```
