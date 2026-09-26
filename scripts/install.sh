#!/bin/sh

set -eu

repository="${COMMANDAGENT_INSTALL_REPOSITORY:-Kewton/CommandAgent}"
api_root="https://api.github.com/repos/$repository"
download_root="https://github.com/$repository/releases/download"
version=""
prefix="${HOME:?HOME must be set}/.local/bin"
temp_dir=""

usage() {
    cat <<'EOF'
Usage: install.sh [--version VERSION] [--prefix DIRECTORY]

Download a released CommandAgent binary, verify its SHA-256 checksum, and
install it into ~/.local/bin or DIRECTORY.

  --version VERSION   Install a release such as 0.1.0 or v0.1.0
                      (default: the latest stable release, or the newest
                      pre-release while no stable release exists)
  --prefix DIRECTORY  Install directly into DIRECTORY
  -h, --help          Show this help
EOF
}

fail() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

cleanup() {
    if [ -n "$temp_dir" ] && [ -d "$temp_dir" ]; then
        rm -rf "$temp_dir"
    fi
}

trap cleanup EXIT HUP INT TERM

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || fail "--version requires a value"
            version=$2
            shift 2
            ;;
        --prefix)
            [ "$#" -ge 2 ] || fail "--prefix requires a directory"
            prefix=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            fail "unknown option: $1"
            ;;
    esac
done

[ -n "$prefix" ] || fail "installation prefix must not be empty"

for command_name in curl tar mkdir mktemp chmod mv tr awk; do
    command -v "$command_name" >/dev/null 2>&1 \
        || fail "required command not found: $command_name"
done

request() {
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        curl -fsSL \
            -H "Accept: application/vnd.github+json" \
            -H "Authorization: Bearer $GITHUB_TOKEN" \
            "$@"
    else
        curl -fsSL -H "Accept: application/vnd.github+json" "$@"
    fi
}

# Print the tag of the first non-draft release in a GitHub releases listing.
# Each release carries exactly one top-level tag_name and draft field; nested
# author and asset objects carry neither, so they pair up in listing order.
newest_published_tag() {
    # shellcheck disable=SC2020 # each separator deliberately maps to a newline
    tr ',{}' '\n\n\n' | awk '
        /^[[:space:]]*"tag_name"[[:space:]]*:/ {
            value = $0
            sub(/^[^:]*:[[:space:]]*"/, "", value)
            sub(/".*$/, "", value)
            tag = value
            have_tag = 1
        }
        /^[[:space:]]*"draft"[[:space:]]*:/ {
            draft = ($0 ~ /:[[:space:]]*true/)
            have_draft = 1
        }
        have_tag && have_draft {
            if (!draft && tag != "") {
                print tag
                exit
            }
            have_tag = 0
            have_draft = 0
        }
    '
}

if [ -z "$version" ]; then
    if release_json="$(request "$api_root/releases/latest" 2>/dev/null)"; then
        tag="$(printf '%s\n' "$release_json" \
            | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
            | sed -n '1p')"
        [ -n "$tag" ] || fail "latest GitHub release did not contain a tag name"
    else
        # GitHub's latest endpoint ignores pre-releases, so fall back to the
        # newest published release while only pre-releases exist.
        releases_json="$(request "$api_root/releases?per_page=30")" \
            || fail "could not resolve the latest CommandAgent release"
        tag="$(printf '%s\n' "$releases_json" | newest_published_tag)"
        [ -n "$tag" ] || fail "no published CommandAgent release was found"
        printf 'No stable release found; using pre-release %s.\n' "$tag" >&2
    fi
    version=${tag#v}
else
    case "$version" in
        v*) tag=$version; version=${version#v} ;;
        *) tag="v$version" ;;
    esac
fi

case "$version" in
    ""|*[!0-9A-Za-z.+-]*) fail "invalid release version: $version" ;;
esac

os="$(uname -s)"
arch="$(uname -m)"
case "$os:$arch" in
    Darwin:arm64|Darwin:aarch64)
        target="aarch64-apple-darwin"
        ;;
    Darwin:x86_64|Darwin:amd64)
        target="x86_64-apple-darwin"
        ;;
    Linux:x86_64|Linux:amd64)
        target="x86_64-unknown-linux-musl"
        ;;
    *)
        fail "unsupported operating system or architecture: $os/$arch"
        ;;
esac

archive="commandagent-$version-$target.tar.gz"
checksum="$archive.sha256"
base_url="$download_root/$tag"

umask 077
temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/commandagent-install.XXXXXX")" \
    || fail "could not create a temporary directory"

printf 'Downloading CommandAgent %s for %s...\n' "$version" "$target"
request -o "$temp_dir/$archive" "$base_url/$archive" \
    || fail "could not download $archive"
request -o "$temp_dir/$checksum" "$base_url/$checksum" \
    || fail "could not download $checksum"

expected_hash="$(sed -n '1{s/[[:space:]].*$//;p;}' "$temp_dir/$checksum")"
case "$expected_hash" in
    ""|*[!0-9A-Fa-f]*) fail "checksum file is malformed" ;;
esac
[ "${#expected_hash}" -eq 64 ] || fail "checksum file is malformed"

if command -v sha256sum >/dev/null 2>&1; then
    actual_hash="$(sha256sum "$temp_dir/$archive" | sed 's/[[:space:]].*$//')"
elif command -v shasum >/dev/null 2>&1; then
    actual_hash="$(shasum -a 256 "$temp_dir/$archive" | sed 's/[[:space:]].*$//')"
else
    fail "sha256sum or shasum is required to verify the release"
fi

[ "$actual_hash" = "$expected_hash" ] \
    || fail "SHA-256 checksum verification failed for $archive"
printf 'Verified SHA-256 checksum.\n'

tar -xzf "$temp_dir/$archive" -C "$temp_dir" commandagent \
    || fail "could not extract commandagent from $archive"
[ -f "$temp_dir/commandagent" ] || fail "release archive did not contain commandagent"
chmod 0755 "$temp_dir/commandagent"

mkdir -p "$prefix" || fail "could not create installation directory: $prefix"
mv "$temp_dir/commandagent" "$prefix/commandagent" \
    || fail "could not install commandagent into $prefix"

printf 'Installed commandagent to %s/commandagent\n' "$prefix"
case ":${PATH:-}:" in
    *":$prefix:"*) ;;
    *)
        printf 'Add CommandAgent to PATH, for example:\n'
        printf '%s\n' "  export PATH=\"$prefix:\$PATH\""
        ;;
esac
