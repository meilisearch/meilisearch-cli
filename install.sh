#!/bin/sh
set -eu

REPO="meilisearch/meilisearch-cli"
BINARY="msc"
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

detect_shell() {
    shell_name="$(basename "${SHELL:-}")"
    case "$shell_name" in
        bash|zsh|fish) echo "$shell_name" ;;
        *) echo "" ;;
    esac
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

install_completions() {
    bin="$1"
    shell="$(detect_shell)"

    if [ -z "$shell" ]; then
        return
    fi

    echo "Installing ${shell} completions..."

    case "$shell" in
        zsh)
            comp_dir="${HOME}/.zfunc"
            mkdir -p "$comp_dir"
            "$bin" completions zsh > "${comp_dir}/_msc"

            # Ensure fpath and compinit are set up in .zshrc
            zshrc="${HOME}/.zshrc"
            if [ -f "$zshrc" ]; then
                if ! grep -q '\.zfunc' "$zshrc" 2>/dev/null; then
                    printf '\n# Meilisearch CLI completions\nfpath=(~/.zfunc $fpath)\nautoload -Uz compinit && compinit\n' >> "$zshrc"
                fi
            fi
            echo "  Installed to ${comp_dir}/_msc"
            ;;
        bash)
            comp_dir="${HOME}/.local/share/bash-completion/completions"
            mkdir -p "$comp_dir"
            "$bin" completions bash > "${comp_dir}/msc"
            echo "  Installed to ${comp_dir}/msc"
            ;;
        fish)
            comp_dir="${HOME}/.config/fish/completions"
            mkdir -p "$comp_dir"
            "$bin" completions fish > "${comp_dir}/msc.fish"
            echo "  Installed to ${comp_dir}/msc.fish"
            ;;
    esac
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

    echo "Installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"

    # Install shell completions
    install_completions "${INSTALL_DIR}/${BINARY}"

    echo ""
    echo "Run 'msc --help' to get started."
    if [ "$(detect_shell)" = "zsh" ]; then
        echo "Restart your shell or run 'source ~/.zshrc' to enable completions."
    fi
}

main "$@"
