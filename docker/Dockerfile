# PMP CLI Docker Image for CI/CD
#
# This image includes PMP CLI and OpenTofu for running infrastructure operations
# in CI/CD pipelines.
#
# Build:
#   docker build -t pmp-cli:latest -f docker/Dockerfile .
#
# Usage:
#   docker run --rm -v $(pwd):/workspace -w /workspace pmp-cli:latest pmp --help

FROM debian:bookworm-slim

LABEL maintainer="PMP Project <https://github.com/pmp-project/pmp-cli>"
LABEL description="PMP CLI with OpenTofu for CI/CD pipelines"

# Build arguments
ARG PMP_VERSION=latest
ARG OPENTOFU_VERSION=1.6.2
ARG TARGETARCH

# Environment variables
ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.pmp/bin:/usr/local/bin:${PATH}"

# Install dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    gnupg \
    jq \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install OpenTofu
RUN ARCH=$(dpkg --print-architecture) && \
    curl -fsSL "https://github.com/opentofu/opentofu/releases/download/v${OPENTOFU_VERSION}/tofu_${OPENTOFU_VERSION}_linux_${ARCH}.deb" -o /tmp/tofu.deb && \
    dpkg -i /tmp/tofu.deb && \
    rm /tmp/tofu.deb && \
    tofu --version

# Install PMP CLI
RUN mkdir -p /root/.pmp/bin && \
    if [ "$PMP_VERSION" = "latest" ]; then \
        curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash; \
    else \
        PMP_VERSION=$PMP_VERSION curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash; \
    fi

# Verify installations
RUN pmp --version && tofu --version

# Set working directory
WORKDIR /workspace

# Default command
CMD ["pmp", "--help"]
