# Template Marketplace

The PMP Template Marketplace allows you to discover, search, and install template packs from multiple registries.

## Commands

### Search for Template Packs

```bash
pmp marketplace search <query>
pmp marketplace search aws
pmp marketplace search networking --registry official
```

### List Available Packs

```bash
pmp marketplace list
pmp marketplace list --registry official
```

### Get Pack Information

```bash
pmp marketplace info <pack-name>
pmp marketplace info aws-networking
```

### Install a Pack

```bash
pmp marketplace install <pack-name>
pmp marketplace install aws-networking
pmp marketplace install aws-networking --version 1.2.0
```

Packs are installed to `~/.pmp/template-packs/`.

### Update Installed Packs

```bash
pmp marketplace update <pack-name>
pmp marketplace update --all
```

## Registry Management

### Add a URL-based Registry

URL registries fetch a JSON index from any URL:

```bash
pmp marketplace registry add official --url https://example.com/registry/index.json
```

### Add a Filesystem Registry

Filesystem registries scan local directories for template packs:

```bash
pmp marketplace registry add local-dev --path ~/my-template-packs
```

### List Registries

```bash
pmp marketplace registry list
```

### Remove a Registry

```bash
pmp marketplace registry remove <name>
```

## Hosting Your Own Registry

You can host your own template pack registry using GitHub Pages or any static file hosting.

### Generate Registry Index

Run this command in a directory containing template packs:

```bash
pmp marketplace generate-index
pmp marketplace generate-index --output ./dist
pmp marketplace generate-index --name my-registry --description "My Template Packs"
```

This generates:
- `index.json` - Machine-readable registry index
- `index.html` - Human-readable pack browser with search

### GitHub Action for Automatic Index Generation

Create `.github/workflows/generate-registry.yml` in your template packs repository:

```yaml
name: Generate Registry Index

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install PMP
        run: |
          curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash
          echo "$HOME/.pmp/bin" >> $GITHUB_PATH

      - name: Generate Index
        run: |
          pmp marketplace generate-index \
            --output ./dist \
            --name "${{ github.repository }}" \
            --description "Template packs for ${{ github.repository }}"

      - name: Setup Pages
        uses: actions/configure-pages@v4

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: ./dist

  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    needs: generate
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

After the workflow runs, your registry will be available at:
```
https://<username>.github.io/<repo>/index.json
```

Users can then add your registry:
```bash
pmp marketplace registry add my-packs --url https://username.github.io/my-repo/index.json
```

## Registry Index Format

The JSON index follows this structure:

```json
{
  "apiVersion": "pmp.io/v1",
  "kind": "RegistryIndex",
  "metadata": {
    "name": "my-registry",
    "description": "My Template Packs",
    "generated_at": "2024-01-15T10:30:00Z"
  },
  "packs": [
    {
      "name": "aws-networking",
      "description": "AWS VPC, subnets, and networking templates",
      "repository": "https://github.com/org/aws-networking-pack",
      "versions": [
        {
          "version": "1.2.0",
          "tag": "v1.2.0",
          "released_at": "2024-01-10T08:00:00Z"
        }
      ],
      "tags": ["aws", "networking", "vpc"],
      "author": "org-name",
      "license": "MIT"
    }
  ]
}
```

## Registry Configuration

Registries are stored in `~/.pmp/registries.yaml`:

```yaml
- apiVersion: pmp.io/v1
  kind: Registry
  metadata:
    name: official
    description: Official PMP Template Packs
  spec:
    source:
      url: https://pmp-project.github.io/registry/index.json
    priority: 100
    enabled: true

- apiVersion: pmp.io/v1
  kind: Registry
  metadata:
    name: local-dev
  spec:
    source:
      path: ~/my-template-packs
    priority: 50
    enabled: true
```

Higher priority registries are searched first. URL-based registries are cached for 1 hour.
