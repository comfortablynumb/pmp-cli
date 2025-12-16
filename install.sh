#!/bin/bash
# PMP CLI Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/pmp-project/pmp-cli/main/install.sh | bash
#
# Environment variables:
#   PMP_VERSION    - Specific version to install (default: latest)
#   PMP_INSTALL_DIR - Installation directory (default: $HOME/.pmp/bin)

set -e

# Configuration
REPO="pmp-project/pmp-cli"
BINARY_NAME="pmp"
DEFAULT_INSTALL_DIR="$HOME/.pmp/bin"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux";;
        Darwin*)    echo "darwin";;
        MINGW*|MSYS*|CYGWIN*) echo "windows";;
        *)          error "Unsupported operating system: $(uname -s)";;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64";;
        arm64|aarch64)  echo "aarch64";;
        *)              error "Unsupported architecture: $(uname -m)";;
    esac
}

# Get latest version from GitHub API
get_latest_version() {
    local latest_url="https://api.github.com/repos/${REPO}/releases/latest"

    if command -v curl &> /dev/null; then
        curl -fsSL "$latest_url" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget &> /dev/null; then
        wget -qO- "$latest_url" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Download file
download() {
    local url="$1"
    local output="$2"

    if command -v curl &> /dev/null; then
        curl -fsSL "$url" -o "$output"
    elif command -v wget &> /dev/null; then
        wget -q "$url" -O "$output"
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Main installation function
main() {
    echo ""
    echo "  ____  __  __ ____   "
    echo " |  _ \|  \/  |  _ \  "
    echo " | |_) | |\/| | |_) | "
    echo " |  __/| |  | |  __/  "
    echo " |_|   |_|  |_|_|     "
    echo ""
    echo " Poor Man's Platform - CLI Installer"
    echo ""

    # Detect platform
    local os=$(detect_os)
    local arch=$(detect_arch)
    info "Detected platform: ${os}-${arch}"

    # Get version
    local version="${PMP_VERSION:-}"
    if [ -z "$version" ]; then
        info "Fetching latest version..."
        version=$(get_latest_version)
        if [ -z "$version" ]; then
            error "Failed to get latest version. Set PMP_VERSION manually or check GitHub releases."
        fi
    fi
    info "Version: ${version}"

    # Set install directory
    local install_dir="${PMP_INSTALL_DIR:-$DEFAULT_INSTALL_DIR}"
    info "Install directory: ${install_dir}"

    # Create install directory
    mkdir -p "$install_dir"

    # Construct download URL
    local ext=""
    if [ "$os" = "windows" ]; then
        ext=".exe"
    fi

    local archive_name="${BINARY_NAME}-${version}-${os}-${arch}"
    local binary_url="https://github.com/${REPO}/releases/download/${version}/${archive_name}.tar.gz"

    # Create temp directory
    local tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    # Download archive
    info "Downloading ${binary_url}..."
    local archive_path="${tmp_dir}/${archive_name}.tar.gz"

    if ! download "$binary_url" "$archive_path" 2>/dev/null; then
        # Try alternative naming convention
        archive_name="${BINARY_NAME}-${os}-${arch}"
        binary_url="https://github.com/${REPO}/releases/download/${version}/${archive_name}.tar.gz"
        info "Trying alternative URL: ${binary_url}..."

        if ! download "$binary_url" "$archive_path" 2>/dev/null; then
            # Try direct binary download (no archive)
            binary_url="https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${os}-${arch}${ext}"
            info "Trying direct binary URL: ${binary_url}..."

            if ! download "$binary_url" "${install_dir}/${BINARY_NAME}${ext}" 2>/dev/null; then
                error "Failed to download PMP binary. Please check:\n  1. Version '${version}' exists\n  2. Release has binaries for ${os}-${arch}\n  3. Network connectivity to github.com"
            fi

            chmod +x "${install_dir}/${BINARY_NAME}${ext}"
            success "Downloaded binary directly"
        else
            # Extract archive
            info "Extracting archive..."
            tar -xzf "$archive_path" -C "$tmp_dir"

            # Find and move binary
            local binary_path=$(find "$tmp_dir" -name "${BINARY_NAME}${ext}" -type f | head -1)
            if [ -z "$binary_path" ]; then
                binary_path=$(find "$tmp_dir" -name "${BINARY_NAME}" -type f | head -1)
            fi

            if [ -z "$binary_path" ]; then
                error "Binary not found in archive"
            fi

            mv "$binary_path" "${install_dir}/${BINARY_NAME}${ext}"
            chmod +x "${install_dir}/${BINARY_NAME}${ext}"
            success "Extracted and installed binary"
        fi
    else
        # Extract archive
        info "Extracting archive..."
        tar -xzf "$archive_path" -C "$tmp_dir"

        # Find and move binary
        local binary_path=$(find "$tmp_dir" -name "${BINARY_NAME}${ext}" -type f | head -1)
        if [ -z "$binary_path" ]; then
            binary_path=$(find "$tmp_dir" -name "${BINARY_NAME}" -type f | head -1)
        fi

        if [ -z "$binary_path" ]; then
            error "Binary not found in archive"
        fi

        mv "$binary_path" "${install_dir}/${BINARY_NAME}${ext}"
        chmod +x "${install_dir}/${BINARY_NAME}${ext}"
        success "Extracted and installed binary"
    fi

    # Verify installation
    if [ -x "${install_dir}/${BINARY_NAME}${ext}" ]; then
        success "PMP installed successfully!"
        echo ""
        info "Binary location: ${install_dir}/${BINARY_NAME}${ext}"

        # Check if install_dir is in PATH
        if [[ ":$PATH:" != *":${install_dir}:"* ]]; then
            echo ""
            warn "Installation directory is not in your PATH."
            echo ""
            echo "Add the following to your shell configuration file:"
            echo ""

            local shell_name=$(basename "$SHELL")
            case "$shell_name" in
                bash)
                    echo "  echo 'export PATH=\"\$HOME/.pmp/bin:\$PATH\"' >> ~/.bashrc"
                    echo "  source ~/.bashrc"
                    ;;
                zsh)
                    echo "  echo 'export PATH=\"\$HOME/.pmp/bin:\$PATH\"' >> ~/.zshrc"
                    echo "  source ~/.zshrc"
                    ;;
                fish)
                    echo "  echo 'set -gx PATH \$HOME/.pmp/bin \$PATH' >> ~/.config/fish/config.fish"
                    ;;
                *)
                    echo "  export PATH=\"\$HOME/.pmp/bin:\$PATH\""
                    ;;
            esac
            echo ""
        fi

        echo "Run 'pmp --help' to get started."
        echo ""
    else
        error "Installation verification failed"
    fi
}

# Run main function
main "$@"
