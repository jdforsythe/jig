#!/usr/bin/env bash
# jig installer — downloads the appropriate binary from GitHub Releases
# Usage: curl -fsSL https://raw.githubusercontent.com/jdforsythe/jig/master/install.sh | sh
# Or: curl -fsSL ... | JIG_VERSION=1.0.0 sh
set -euo pipefail

REPO="jdforsythe/jig"
INSTALL_DIR="${JIG_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${JIG_VERSION:-}"

detect_platform() {
    local os arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "${os}-${arch}" in
        darwin-arm64)   echo "jig-macos-arm64" ;;
        darwin-x86_64)  echo "jig-macos-x86_64" ;;
        linux-x86_64)   echo "jig-linux-x86_64" ;;
        linux-aarch64)  echo "jig-linux-aarch64" ;;
        linux-arm64)    echo "jig-linux-aarch64" ;;
        *)
            echo "Unsupported platform: ${os}-${arch}" >&2
            echo "Please download manually from https://github.com/${REPO}/releases" >&2
            exit 1
            ;;
    esac
}

get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | sed 's/.*"v\([^"]*\)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | sed 's/.*"v\([^"]*\)".*/\1/'
    else
        echo "Error: curl or wget is required" >&2
        exit 1
    fi
}

main() {
    local artifact version url

    artifact=$(detect_platform)

    if [ -z "$VERSION" ]; then
        echo "Fetching latest version..."
        VERSION=$(get_latest_version)
        if [ -z "$VERSION" ]; then
            echo "Error: Could not determine latest version" >&2
            exit 1
        fi
    fi

    version="$VERSION"
    url="https://github.com/${REPO}/releases/download/v${version}/${artifact}"

    echo "Installing jig v${version} (${artifact})..."
    echo "Download URL: ${url}"

    mkdir -p "$INSTALL_DIR"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --progress-bar "$url" -o "$INSTALL_DIR/jig"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --show-progress "$url" -O "$INSTALL_DIR/jig"
    else
        echo "Error: curl or wget is required" >&2
        exit 1
    fi

    chmod +x "$INSTALL_DIR/jig"

    echo ""
    echo "jig v${version} installed to ${INSTALL_DIR}/jig"

    if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
        echo ""
        echo "Note: Add ${INSTALL_DIR} to your PATH:"
        echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
        echo "  # or for zsh:"
        echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
    fi

    echo ""
    echo "Verify installation:"
    echo "  jig --version"
}

main "$@"
