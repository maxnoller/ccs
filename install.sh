#!/bin/bash
set -e

# CCS Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/maxnoller/ccs/main/install.sh | bash

REPO="maxnoller/ccs"
INSTALL_DIR="${CCS_INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="darwin" ;;
        *)       error "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)            error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${os}-${arch}"
}

# Get latest release version
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install binary
install_binary() {
    local version="$1"
    local platform="$2"
    local asset_name="ccs-${platform}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/${version}/${asset_name}"

    info "Downloading ccs ${version} for ${platform}..."

    local tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    if ! curl -fsSL "$download_url" -o "$tmp_dir/ccs.tar.gz" 2>/dev/null; then
        return 1
    fi

    tar -xzf "$tmp_dir/ccs.tar.gz" -C "$tmp_dir"

    mkdir -p "$INSTALL_DIR"
    mv "$tmp_dir/ccs" "$INSTALL_DIR/ccs"
    chmod +x "$INSTALL_DIR/ccs"

    return 0
}

# Build from source
build_from_source() {
    info "Building from source..."

    if ! command -v cargo &>/dev/null; then
        error "Rust/Cargo not found. Install from https://rustup.rs"
    fi

    local tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    git clone --depth 1 "https://github.com/${REPO}.git" "$tmp_dir/ccs"
    cd "$tmp_dir/ccs"

    cargo build --release

    mkdir -p "$INSTALL_DIR"
    mv target/release/ccs "$INSTALL_DIR/ccs"
    chmod +x "$INSTALL_DIR/ccs"
}

# Check if install dir is in PATH
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "Add $INSTALL_DIR to your PATH:"
        echo ""
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
        echo "Add this line to your ~/.bashrc, ~/.zshrc, or shell config."
    fi
}

main() {
    info "Installing ccs (Claude Code Sandbox)..."

    local platform=$(detect_platform)
    info "Detected platform: $platform"

    local version=$(get_latest_version)

    if [ -n "$version" ]; then
        info "Latest version: $version"

        if install_binary "$version" "$platform"; then
            info "Successfully installed ccs to $INSTALL_DIR/ccs"
        else
            warn "Pre-built binary not available for $platform"
            build_from_source
            info "Successfully built and installed ccs to $INSTALL_DIR/ccs"
        fi
    else
        warn "Could not determine latest version, building from source"
        build_from_source
        info "Successfully built and installed ccs to $INSTALL_DIR/ccs"
    fi

    check_path

    echo ""
    info "Installation complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Build the Docker image: ccs --build"
    echo "  2. Run in a project:       cd ~/myproject && ccs"
    echo "  3. Check status:           ccs --status"
    echo ""
}

main "$@"
