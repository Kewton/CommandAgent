#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(CDPATH='' cd -- "$script_dir/.." && pwd -P)"
target_root="$repo_root/target"
release_dir="$target_root/release"
build_dir=""
publish_dir=""

cleanup() {
    local status=$?
    trap - EXIT
    if [[ -n "$build_dir" ]]; then
        rm -rf -- "$build_dir"
    fi
    if [[ -n "$publish_dir" ]]; then
        rm -rf -- "$publish_dir"
    fi
    exit "$status"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' HUP TERM

mkdir -p -- "$target_root"
build_dir="$(mktemp -d "$target_root/.commandagent-release-build.XXXXXX")"

package_version="$({
    awk '
        /^\[package\][[:space:]]*$/ { in_package = 1; next }
        /^\[/ { in_package = 0 }
        in_package && /^[[:space:]]*version[[:space:]]*=/ {
            if (match($0, /"[^"]+"/)) {
                print substr($0, RSTART + 1, RLENGTH - 2)
                exit
            }
        }
    ' "$repo_root/Cargo.toml"
} )"
if [[ -z "$package_version" ]]; then
    echo "error: could not read the package version from Cargo.toml" >&2
    exit 1
fi

commit="$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || true)"
if [[ -z "$commit" ]]; then
    commit="unknown"
fi
dirty_suffix=""
git_status="$(git -C "$repo_root" status --porcelain 2>/dev/null || true)"
if [[ -n "$git_status" ]]; then
    dirty_suffix="+dirty"
fi

(
    cd -- "$repo_root"
    CARGO_TARGET_DIR="$build_dir" cargo build --release --locked --bin commandagent
)

candidate="$build_dir/release/commandagent"
if [[ ! -x "$candidate" ]]; then
    echo "error: release build did not produce an executable commandagent binary" >&2
    exit 1
fi

version_output="$($candidate --version)"
read -r binary_name built_version built_commit built_timestamp extra <<<"$version_output"
if [[ "$binary_name" != "commandagent" \
    || "$built_version" != "$package_version" \
    || "$built_commit" != "$commit$dirty_suffix" \
    || -z "$built_timestamp" \
    || -n "${extra:-}" ]]; then
    echo "error: staged commandagent has unexpected version provenance" >&2
    echo "expected: commandagent $package_version $commit$dirty_suffix <timestamp>" >&2
    echo "actual:   $version_output" >&2
    exit 1
fi

publish_dir="$(mktemp -d "$target_root/.commandagent-release-publish.XXXXXX")"
cp -- "$candidate" "$publish_dir/commandagent"
chmod 0755 "$publish_dir/commandagent"

mkdir -p -- "$release_dir"
find "$release_dir" -mindepth 1 -maxdepth 1 ! -name commandagent -exec rm -rf -- {} +
mv -f -- "$publish_dir/commandagent" "$release_dir/commandagent"

printf 'Published %s\n%s\n' "$release_dir/commandagent" "$version_output"
