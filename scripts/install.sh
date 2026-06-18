#!/usr/bin/env bash
set -euo pipefail

repo="${COMMANDAGENT_REPO:-Kewton/CommandAgent}"
version="${COMMANDAGENT_VERSION:-latest}"
install_dir="${COMMANDAGENT_INSTALL_DIR:-$HOME/.local/bin}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: $1 is required" >&2
    exit 2
  fi
}

asset_name() {
  local os
  local arch

  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os:$arch" in
    Linux:x86_64 | Linux:amd64)
      echo "commandagent-linux-amd64"
      ;;
    Linux:aarch64 | Linux:arm64)
      echo "commandagent-linux-arm64"
      ;;
    Darwin:x86_64 | Darwin:amd64)
      echo "commandagent-darwin-amd64"
      ;;
    Darwin:aarch64 | Darwin:arm64)
      echo "commandagent-darwin-arm64"
      ;;
    *)
      echo "error: unsupported platform: $os $arch" >&2
      exit 2
      ;;
  esac
}

download_base_url() {
  if [[ "$version" == "latest" ]]; then
    echo "https://github.com/$repo/releases/latest/download"
  else
    echo "https://github.com/$repo/releases/download/$version"
  fi
}

verify_checksum() {
  local checksum_file="$1"

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$checksum_file"
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$checksum_file"
    return
  fi

  echo "error: shasum or sha256sum is required for checksum verification" >&2
  exit 2
}

need_cmd curl
need_cmd gzip
need_cmd install
need_cmd mktemp
need_cmd uname

asset="$(asset_name)"
base_url="$(download_base_url)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

curl -fsSL "$base_url/$asset.gz" -o "$tmp_dir/$asset.gz"
curl -fsSL "$base_url/$asset.gz.sha256" -o "$tmp_dir/$asset.gz.sha256"

(
  cd "$tmp_dir"
  verify_checksum "$asset.gz.sha256"
)

gzip -dc "$tmp_dir/$asset.gz" > "$tmp_dir/commandagent"
mkdir -p "$install_dir"
install -m 0755 "$tmp_dir/commandagent" "$install_dir/commandagent"

echo "Installed commandagent to $install_dir/commandagent"

if ! command -v commandagent >/dev/null 2>&1; then
  echo "Add $install_dir to PATH if commandagent is not found by your shell."
fi
