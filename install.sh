#!/bin/sh
set -eu

REPO="meilisearch/meilisearch-cli"
BINARY="meilisearch"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
detect_platform() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      echo "Unsupported OS: $os" >&2; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        arm64|aarch64)   arch="aarch64" ;;
        *)               echo "Unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    echo "${arch}-${os}"
}

download() {
    url="$1"
    dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$url"
    else
        echo "Error: curl or wget is required" >&2
        exit 1
    fi
}

main() {
    platform="$(detect_platform)"
    version="${1:-latest}"

    echo "Installing ${BINARY} (${version}) for ${platform}..."

    archive_name="${BINARY}-${version}-${platform}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    echo "Downloading ${url}..."
    download "$url" "${tmpdir}/${archive_name}"

    echo "Extracting..."
    tar -xzf "${tmpdir}/${archive_name}" -C "$tmpdir"

    # Find the binary in the extracted archive
    bin_path="$(find "$tmpdir" -name "$BINARY" -type f | head -1)"
    if [ -z "$bin_path" ]; then
        echo "Error: binary not found in archive" >&2
        exit 1
    fi

    chmod +x "$bin_path"

    if [ -w "$INSTALL_DIR" ]; then
        mv "$bin_path" "${INSTALL_DIR}/${BINARY}"
    else
        echo "Installing to ${INSTALL_DIR} (requires sudo)..."
        sudo mv "$bin_path" "${INSTALL_DIR}/${BINARY}"
    fi

    echo ""
    echo "Installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"
    "${INSTALL_DIR}/${BINARY}" --version 2>/dev/null || true
    echo ""
    echo "Run 'meilisearch --help' to get started."
}

main "$@"
