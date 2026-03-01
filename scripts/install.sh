#!/usr/bin/env bash
set -euo pipefail

REPO="shonenada-vibe/mdw"
INSTALL_DIR="${MDW_INSTALL_DIR:-/usr/local/bin}"
BINARY="mdw"

info() { printf '\033[1;34m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31merror: %s\033[0m\n' "$*" >&2; exit 1; }

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)  os="unknown-linux-gnu" ;;
    Darwin) os="apple-darwin" ;;
    *)      error "Unsupported OS: $os" ;;
  esac

  case "$arch" in
    x86_64|amd64)  arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *)             error "Unsupported architecture: $arch" ;;
  esac

  if [ "$os" = "unknown-linux-gnu" ] && [ "$arch" = "aarch64" ]; then
    error "Pre-built binaries are not available for Linux aarch64 yet"
  fi

  echo "${arch}-${os}"
}

fetch() {
  local url="$1" dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url"
  else
    error "curl or wget is required"
  fi
}

get_latest_tag() {
  local url="https://api.github.com/repos/${REPO}/releases/latest"
  local tag
  if command -v curl >/dev/null 2>&1; then
    tag=$(curl -fsSL "$url" | grep '"tag_name"' | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/')
  elif command -v wget >/dev/null 2>&1; then
    tag=$(wget -qO- "$url" | grep '"tag_name"' | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/')
  else
    error "curl or wget is required"
  fi
  [ -z "$tag" ] && error "Failed to determine latest release tag"
  echo "$tag"
}

verify_checksum() {
  local file="$1" expected="$2"
  local actual
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$file" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$file" | awk '{print $1}')
  else
    info "Warning: sha256sum/shasum not found, skipping checksum verification"
    return 0
  fi
  if [ "$actual" != "$expected" ]; then
    error "Checksum mismatch (expected ${expected}, got ${actual})"
  fi
}

main() {
  local tag="${1:-}"
  if [ -z "$tag" ]; then
    info "Fetching latest release tag..."
    tag=$(get_latest_tag)
  fi
  info "Installing mdw ${tag}"

  local target
  target=$(detect_platform)
  info "Detected platform: ${target}"

  local archive="mdw-${tag}-${target}.tar.gz"
  local base_url="https://github.com/${REPO}/releases/download/${tag}"
  local tmpdir
  tmpdir=$(mktemp -d)
  trap 'rm -rf "$tmpdir"' EXIT

  info "Downloading ${archive}..."
  fetch "${base_url}/${archive}" "${tmpdir}/${archive}"
  fetch "${base_url}/${archive}.sha256" "${tmpdir}/${archive}.sha256"

  info "Verifying checksum..."
  local expected_sha
  expected_sha=$(awk '{print $1}' "${tmpdir}/${archive}.sha256")
  verify_checksum "${tmpdir}/${archive}" "$expected_sha"

  info "Extracting binary..."
  tar xzf "${tmpdir}/${archive}" -C "${tmpdir}"

  info "Installing to ${INSTALL_DIR}..."
  if [ -w "$INSTALL_DIR" ]; then
    install -m 755 "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  else
    info "Elevated permissions required to install to ${INSTALL_DIR}"
    sudo install -m 755 "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  fi

  info "mdw ${tag} installed successfully to ${INSTALL_DIR}/${BINARY}"
}

main "$@"
