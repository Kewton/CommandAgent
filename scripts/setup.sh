#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(CDPATH='' cd -- "$script_dir/.." && pwd -P)"
quickstart_link="README.md#quickstart"
ollama_url="http://localhost:11434"
minimum_node_version="20.9.0"

mode="interactive"
check_only=false
gui_enabled=false
gui_base_path="/"
extension_root=""
write_config=false
gui_token_file=""
profile_set=""
current_step="initialization"
commandagent_bin=""
package_version=""
source_commit=""
expected_build_commit=""
node_ready=false
ollama_installed=false
ollama_reachable=false

summary_steps=()
summary_states=()
summary_details=()

usage() {
    cat <<'EOF'
Usage: ./scripts/setup.sh [--yes | --check-only] [GUI/config options]

  no arguments   Interactive setup with confirmation at each optional step
  --yes          Non-interactive setup using safe defaults
  --check-only   Check prerequisites without changing anything
  --gui          Install dependencies and build the management GUI
  --base-path PATH
                 Build and serve the GUI below PATH (default: /)
  --extension-root DIR
                 Create the private extension-root skeleton at DIR
  --write-config Write .commandagent/config.toml when it does not exist
  --gui-token-file PATH
                 Write a new private GUI Trial token without displaying it
  --profile-set PROFILE
                 Check profile prerequisites (nextjs or python-cli)
EOF
}

add_summary() {
    local index=${#summary_steps[@]}
    summary_steps[index]="$1"
    summary_states[index]="$2"
    summary_details[index]="$3"
}

print_summary() {
    local index
    printf '\nSetup summary\n'
    for ((index = 0; index < ${#summary_steps[@]}; index += 1)); do
        printf -- '- %-24s %-7s %s\n' \
            "${summary_steps[$index]}" \
            "${summary_states[$index]}" \
            "${summary_details[$index]}"
    done
    printf '\nNext: follow the README Quickstart at %s\n' "$quickstart_link"
}

fail() {
    trap - ERR
    printf 'error: %s\n' "$1" >&2
    exit 1
}

unexpected_failure() {
    local status=$?
    local line=$1
    trap - ERR
    printf 'error: %s failed near line %s; complete that step manually, then rerun ./scripts/setup.sh\n' \
        "$current_step" "$line" >&2
    exit "$status"
}

interrupted() {
    trap - HUP INT TERM
    printf 'error: %s was interrupted; finish it manually if needed, then rerun ./scripts/setup.sh\n' \
        "$current_step" >&2
    exit 130
}

trap 'unexpected_failure "$LINENO"' ERR
trap interrupted HUP INT TERM

while [[ $# -gt 0 ]]; do
    case "$1" in
        --yes)
            [[ "$mode" == "interactive" ]] || fail "--yes and --check-only cannot be combined"
            mode="yes"
            ;;
        --check-only)
            [[ "$mode" == "interactive" ]] || fail "--yes and --check-only cannot be combined"
            mode="check-only"
            check_only=true
            ;;
        --gui) gui_enabled=true ;;
        --write-config) write_config=true ;;
        --base-path|--extension-root|--gui-token-file|--profile-set)
            [[ $# -ge 2 && -n "$2" ]] || fail "$1 requires a value"
            case "$1" in
                --base-path) gui_base_path=$2 ;;
                --extension-root) extension_root=$2 ;;
                --gui-token-file) gui_token_file=$2 ;;
                --profile-set) profile_set=$2 ;;
            esac
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            usage >&2
            fail "unknown option '$1'; rerun ./scripts/setup.sh --help"
            ;;
    esac
    shift
done

case "$profile_set" in
    ""|nextjs|python-cli) ;;
    *) fail "--profile-set must be nextjs or python-cli" ;;
esac
if [[ "$gui_base_path" != /* || "$gui_base_path" == *"//"* || "$gui_base_path" == *".."* \
    || "$gui_base_path" == *"?"* || "$gui_base_path" == *"#"* ]]; then
    fail "--base-path must be '/' or an absolute path"
fi
if [[ "$gui_base_path" != "/" ]]; then
    gui_base_path=${gui_base_path%/}
fi

confirm() {
    local prompt=$1
    local answer=""
    if [[ "$mode" == "yes" ]]; then
        return 0
    fi
    printf '%s [y/N] ' "$prompt"
    if ! IFS= read -r answer; then
        answer=""
    fi
    case "$answer" in
        y|Y|yes|YES|Yes) return 0 ;;
        *) return 1 ;;
    esac
}

version_at_least() {
    local current=${1%%-*}
    local required=${2%%-*}
    local current_major=""
    local current_minor=""
    local current_patch=""
    local required_major=""
    local required_minor=""
    local required_patch=""

    IFS=. read -r current_major current_minor current_patch <<<"$current"
    IFS=. read -r required_major required_minor required_patch <<<"$required"
    current_patch=${current_patch:-0}
    required_patch=${required_patch:-0}

    case "$current_major$current_minor$current_patch$required_major$required_minor$required_patch" in
        *[!0-9]*) return 1 ;;
    esac

    if ((current_major != required_major)); then
        ((current_major > required_major))
    elif ((current_minor != required_minor)); then
        ((current_minor > required_minor))
    else
        ((current_patch >= required_patch))
    fi
}

manifest_value() {
    local key=$1
    local line=""
    line="$(grep -E -m 1 "^[[:space:]]*${key}[[:space:]]*=" "$repo_root/Cargo.toml" || true)"
    printf '%s\n' "$line" | sed -E 's/^[^"]*"([^"]+)".*$/\1/'
}

key_is_configured() {
    local key=$1
    case "$key" in
        GEMINI_API_KEY)
            [[ -n "${GEMINI_API_KEY:-}" ]] && return 0
            ;;
        OPENAI_API_KEY)
            [[ -n "${OPENAI_API_KEY:-}" ]] && return 0
            ;;
    esac
    [[ -f "$repo_root/.env" ]] && grep -q -E "^[[:space:]]*${key}=" "$repo_root/.env"
}

check_prerequisites() {
    local required_rust=""
    local rust_output=""
    local rust_version=""
    local required_failures=0
    local required_detail="cargo, rustc, and git available"
    local node_detail=""
    local node_version=""
    local ollama_detail=""
    local python_detail=""
    local api_detail=""

    current_step="prerequisite checks"
    printf 'Checking prerequisites (no changes are made)...\n'

    if [[ ! -f "$repo_root/Cargo.toml" ]]; then
        fail "Cargo.toml is missing; run this script from a complete CommandAgent checkout"
    fi
    required_rust="$(manifest_value rust-version)"
    if [[ -z "$required_rust" || "$required_rust" == *'='* ]]; then
        fail "could not read rust-version from Cargo.toml; inspect the manifest and retry"
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        printf 'required: cargo is missing; install Rust and Cargo from https://www.rust-lang.org/tools/install\n' >&2
        required_failures=$((required_failures + 1))
    elif ! cargo --version >/dev/null 2>&1; then
        printf 'required: cargo could not run; repair it using https://www.rust-lang.org/tools/install\n' >&2
        required_failures=$((required_failures + 1))
    fi

    if ! command -v rustc >/dev/null 2>&1; then
        printf 'required: rustc is missing; install Rust %s or newer from https://www.rust-lang.org/tools/install\n' \
            "$required_rust" >&2
        required_failures=$((required_failures + 1))
    else
        rust_output="$(rustc --version 2>/dev/null || true)"
        rust_version=${rust_output#rustc }
        rust_version=${rust_version%% *}
        if [[ -z "$rust_output" ]] || ! version_at_least "$rust_version" "$required_rust"; then
            printf 'required: rustc %s is older than Cargo.toml rust-version %s; update via https://www.rust-lang.org/tools/install\n' \
                "${rust_version:-unknown}" "$required_rust" >&2
            required_failures=$((required_failures + 1))
        else
            printf 'ok: rustc %s satisfies rust-version %s\n' "$rust_version" "$required_rust"
        fi
    fi

    if ! command -v git >/dev/null 2>&1; then
        printf 'required: git is missing; install it from https://git-scm.com/downloads\n' >&2
        required_failures=$((required_failures + 1))
    elif ! git --version >/dev/null 2>&1; then
        printf 'required: git could not run; repair it using https://git-scm.com/downloads\n' >&2
        required_failures=$((required_failures + 1))
    fi

    if ((required_failures == 0)); then
        add_summary "Required tools" "ok" "$required_detail"
    else
        add_summary "Required tools" "warn" "$required_failures required check(s) failed"
    fi

    if command -v node >/dev/null 2>&1 && command -v npm >/dev/null 2>&1; then
        node_version="$(node --version 2>/dev/null || true)"
        node_version=${node_version#v}
        if version_at_least "$node_version" "$minimum_node_version"; then
            node_ready=true
            node_detail="node $node_version and npm available"
            printf 'ok: node %s and npm are available for GUI and interaction probes\n' "$node_version"
            add_summary "Node/npm (optional)" "ok" "$node_detail"
        elif [[ "$gui_enabled" == true || "$profile_set" == "nextjs" ]]; then
            node_detail="node ${node_version:-unknown} is older than required $minimum_node_version"
            printf 'required: %s; update from https://nodejs.org/\n' "$node_detail" >&2
            add_summary "Node/npm (GUI)" "warn" "$node_detail"
            required_failures=$((required_failures + 1))
        else
            node_detail="node ${node_version:-unknown} is older than GUI minimum $minimum_node_version"
            printf 'warning: %s\n' "$node_detail" >&2
            add_summary "Node/npm (optional)" "warn" "$node_detail"
        fi
    else
        node_detail="node and/or npm missing; install from https://nodejs.org/ to enable the interaction probe"
        if [[ "$gui_enabled" == true || "$profile_set" == "nextjs" ]]; then
            printf 'required: %s\n' "$node_detail" >&2
            add_summary "Node/npm (GUI)" "warn" "$node_detail"
            required_failures=$((required_failures + 1))
        else
            printf 'warning: %s\n' "$node_detail" >&2
            add_summary "Node/npm (optional)" "warn" "$node_detail"
        fi
    fi
    if command -v ollama >/dev/null 2>&1; then
        ollama_installed=true
    fi
    if command -v curl >/dev/null 2>&1 \
        && curl --silent --fail --max-time 2 "$ollama_url/api/tags" >/dev/null 2>&1; then
        ollama_reachable=true
    fi
    if [[ "$ollama_installed" == true && "$ollama_reachable" == true ]]; then
        ollama_detail="ollama installed and $ollama_url reachable"
        add_summary "Ollama (optional)" "ok" "$ollama_detail"
    elif [[ "$ollama_installed" == true ]]; then
        ollama_detail="ollama installed; start it to make $ollama_url reachable"
        printf 'warning: %s\n' "$ollama_detail" >&2
        add_summary "Ollama (optional)" "warn" "$ollama_detail"
    elif [[ "$ollama_reachable" == true ]]; then
        ollama_detail="$ollama_url reachable without the ollama CLI"
        add_summary "Ollama (optional)" "ok" "$ollama_detail"
    else
        ollama_detail="ollama unavailable; install from https://ollama.com/download or start $ollama_url"
        printf 'warning: %s\n' "$ollama_detail" >&2
        add_summary "Ollama (optional)" "warn" "$ollama_detail"
    fi

    if command -v python3 >/dev/null 2>&1; then
        python_detail="python3 available for evaluation tooling"
        add_summary "Python (optional)" "ok" "$python_detail"
    else
        python_detail="python3 missing; install from https://www.python.org/downloads/ for evaluation tooling"
        if [[ "$profile_set" == "python-cli" ]]; then
            printf 'required: %s\n' "$python_detail" >&2
            add_summary "Python (python-cli)" "warn" "$python_detail"
            required_failures=$((required_failures + 1))
        else
            printf 'warning: %s\n' "$python_detail" >&2
            add_summary "Python (optional)" "warn" "$python_detail"
        fi
    fi

    if key_is_configured GEMINI_API_KEY || key_is_configured OPENAI_API_KEY; then
        api_detail="at least one remote-provider key is configured"
        add_summary "API keys (optional)" "ok" "$api_detail"
    else
        api_detail="no remote-provider key configured; local Ollama use is unaffected"
        printf 'warning: %s\n' "$api_detail" >&2
        add_summary "API keys (optional)" "warn" "$api_detail"
    fi

    if ((required_failures > 0)); then
        printf 'error: prerequisite checks failed; install or repair the required tools above, then rerun ./scripts/setup.sh\n' >&2
        print_summary
        exit 1
    fi
}

resolve_source_provenance() {
    local git_status=""
    current_step="source provenance detection"
    package_version="$(manifest_value version)"
    if [[ -z "$package_version" || "$package_version" == *'='* ]]; then
        fail "could not read the package version from Cargo.toml; inspect the manifest and retry"
    fi
    source_commit="$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || true)"
    if [[ -z "$source_commit" ]]; then
        fail "Git could not identify this checkout; run cargo install --path . --locked manually from a valid clone"
    fi
    git_status="$(git -C "$repo_root" status --porcelain 2>/dev/null || true)"
    expected_build_commit="$source_commit"
    if [[ -n "$git_status" ]]; then
        expected_build_commit="${source_commit}+dirty"
    fi
}

binary_matches_source() {
    local candidate=$1
    local output=""
    local binary_name=""
    local binary_version=""
    local binary_commit=""
    local binary_timestamp=""
    local extra=""
    [[ -x "$candidate" ]] || return 1
    output="$("$candidate" --version 2>/dev/null || true)"
    read -r binary_name binary_version binary_commit binary_timestamp extra <<<"$output"
    [[ "$binary_name" == "commandagent" \
        && "$binary_version" == "$package_version" \
        && "$binary_commit" == "$expected_build_commit" \
        && -n "$binary_timestamp" \
        && -z "$extra" ]]
}

find_commandagent() {
    local candidate=""
    candidate="$(command -v commandagent 2>/dev/null || true)"
    if [[ -n "$candidate" && -x "$candidate" ]]; then
        commandagent_bin="$candidate"
        return 0
    fi
    candidate="${CARGO_HOME:-$HOME/.cargo}/bin/commandagent"
    if [[ -x "$candidate" ]]; then
        commandagent_bin="$candidate"
        return 0
    fi
    return 1
}

install_or_build() {
    local existing=""
    local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin/commandagent"
    current_step="CommandAgent install"
    resolve_source_provenance

    if find_commandagent; then
        existing="$commandagent_bin"
        if binary_matches_source "$existing"; then
            printf 'skipped: matching CommandAgent build is already installed at %s\n' "$existing"
            add_summary "Build/install" "skipped" "matching source build already available"
            return
        fi
    fi
    if [[ "$cargo_bin" != "$existing" ]] && binary_matches_source "$cargo_bin"; then
        commandagent_bin="$cargo_bin"
        printf 'skipped: matching CommandAgent build is already installed at %s\n' "$cargo_bin"
        printf 'PATH suggestion: add %s to PATH to invoke commandagent by name\n' "${CARGO_HOME:-$HOME/.cargo}/bin"
        add_summary "Build/install" "skipped" "matching Cargo binary already available; PATH advice shown"
        return
    fi

    if confirm "Install CommandAgent with 'cargo install --path . --locked'?"; then
        if ! (cd -- "$repo_root" && cargo install --path . --locked); then
            fail "cargo install failed; run 'cargo install --path . --locked' manually from $repo_root"
        fi
        if [[ -x "$cargo_bin" ]]; then
            commandagent_bin="$cargo_bin"
        else
            commandagent_bin=""
            find_commandagent || true
        fi
        if [[ -z "$commandagent_bin" ]]; then
            fail "cargo install completed but commandagent was not found; add ${CARGO_HOME:-$HOME/.cargo}/bin to PATH and run 'commandagent --version'"
        fi
        if ! binary_matches_source "$commandagent_bin"; then
            fail "cargo install completed but the installed binary does not match this source; rerun 'cargo install --path . --locked --force' manually"
        fi
        add_summary "Build/install" "ok" "installed with cargo install --path . --locked"
    else
        current_step="CommandAgent release build fallback"
        printf 'Install declined; building target/release/commandagent instead.\n'
        if ! (cd -- "$repo_root" && cargo build --release); then
            fail "release build failed; run 'cargo build --release' manually from $repo_root"
        fi
        commandagent_bin="$repo_root/target/release/commandagent"
        if [[ ! -x "$commandagent_bin" ]]; then
            fail "release build produced no executable; rerun 'cargo build --release' and inspect Cargo output"
        fi
        printf 'PATH suggestion: export PATH="%s/target/release:%s"\n' "$repo_root" "\$PATH"
        add_summary "Build/install" "ok" "release build created; PATH addition shown above"
    fi
}

setup_shell_completion() {
    local shell_name="${SHELL##*/}"
    local completion_dir=""
    local completion_file=""
    local temporary_file=""
    local activation=""
    current_step="shell completion setup"

    case "$shell_name" in
        bash)
            completion_dir="${BASH_COMPLETION_USER_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion}/completions"
            completion_file="$completion_dir/commandagent"
            activation="Restart Bash or source the generated file."
            ;;
        zsh)
            completion_dir="${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"
            completion_file="$completion_dir/_commandagent"
            activation="Add 'fpath=(\"$completion_dir\" \$fpath)' before 'autoload -Uz compinit && compinit' in .zshrc."
            ;;
        fish)
            completion_dir="${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions"
            completion_file="$completion_dir/commandagent.fish"
            activation="Restart Fish; it loads files in the completions directory automatically."
            ;;
        *)
            printf 'skipped: shell completion auto-install supports Bash, Zsh, and Fish; see docs/guide/en/cli-reference.md for manual commands\n'
            add_summary "Shell completion" "skipped" "unsupported or unknown SHELL=${SHELL:-unset}; manual guide available"
            return
            ;;
    esac

    if ! confirm "Install commandagent completion for $shell_name at $completion_file?"; then
        printf 'Manual completion: %s --completions %s > %s\n' \
            "$commandagent_bin" "$shell_name" "$completion_file"
        add_summary "Shell completion" "skipped" "installation declined; manual command shown"
        return
    fi

    if ! mkdir -p -- "$completion_dir"; then
        printf 'warning: could not create %s; install the %s completion manually\n' \
            "$completion_dir" "$shell_name" >&2
        add_summary "Shell completion" "warn" "completion directory could not be created"
        return
    fi
    temporary_file="${completion_file}.tmp.$$"
    if ! "$commandagent_bin" --completions "$shell_name" >"$temporary_file"; then
        rm -f -- "$temporary_file"
        printf 'warning: completion generation failed; run commandagent --completions %s manually\n' \
            "$shell_name" >&2
        add_summary "Shell completion" "warn" "generation failed; manual command required"
        return
    fi
    if ! mv -- "$temporary_file" "$completion_file"; then
        rm -f -- "$temporary_file"
        printf 'warning: could not install %s; generate it manually\n' "$completion_file" >&2
        add_summary "Shell completion" "warn" "generated file could not be installed"
        return
    fi

    printf 'ok: installed %s completion at %s\n' "$shell_name" "$completion_file"
    printf '%s\n' "$activation"
    add_summary "Shell completion" "ok" "$shell_name completion installed; activation advice shown"
}

append_api_key() {
    local key=$1
    local provider=$2
    local env_file="$repo_root/.env"
    local secret_value=""
    local created=false

    if key_is_configured "$key"; then
        printf 'skipped: %s is already configured; its value was not read or changed\n' "$key"
        return 2
    fi
    if ! confirm "Configure $provider with $key in .env?"; then
        return 2
    fi

    printf 'Enter %s (input hidden): ' "$key"
    if ! IFS= read -r -s secret_value; then
        secret_value=""
    fi
    printf '\n'
    if [[ -z "$secret_value" ]]; then
        printf 'warning: %s was left empty; set it manually in .env or the process environment\n' "$key" >&2
        unset secret_value
        return 3
    fi

    if [[ -L "$env_file" ]]; then
        printf 'warning: .env is a symbolic link; set %s manually rather than following the link\n' "$key" >&2
        unset secret_value
        return 3
    fi
    if [[ ! -e "$env_file" ]]; then
        if ! (umask 177 && : >"$env_file"); then
            unset secret_value
            printf 'warning: .env creation failed; set %s in the process environment manually\n' "$key" >&2
            return 3
        fi
        created=true
    elif [[ ! -f "$env_file" ]]; then
        unset secret_value
        printf 'warning: .env is not a regular file; set %s in the process environment manually\n' "$key" >&2
        return 3
    fi

    if [[ -s "$env_file" ]]; then
        if ! printf '\n%s=%s\n' "$key" "$secret_value" >>"$env_file"; then
            unset secret_value
            printf 'warning: writing %s failed; set it in .env or the process environment manually\n' "$key" >&2
            return 3
        fi
    elif ! printf '%s=%s\n' "$key" "$secret_value" >>"$env_file"; then
        unset secret_value
        printf 'warning: writing %s failed; set it in .env or the process environment manually\n' "$key" >&2
        return 3
    fi
    unset secret_value
    if [[ "$created" == true ]]; then
        chmod 600 "$env_file"
    fi
    printf 'ok: added %s to .env without changing existing entries\n' "$key"
    return 0
}

configure_api_keys() {
    local configured=0
    local skipped=0
    local warned=0
    local result=0
    current_step="API key setup"

    if [[ "$mode" == "yes" ]]; then
        printf 'skipped: --yes never prompts for or writes API keys; set GEMINI_API_KEY or OPENAI_API_KEY manually if needed\n'
        add_summary "API key setup" "skipped" "non-interactive mode never writes secrets"
        return
    fi

    append_api_key GEMINI_API_KEY Gemini || result=$?
    case "$result" in
        0) configured=$((configured + 1)) ;;
        2) skipped=$((skipped + 1)) ;;
        *) warned=$((warned + 1)) ;;
    esac
    result=0
    append_api_key OPENAI_API_KEY OpenAI || result=$?
    case "$result" in
        0) configured=$((configured + 1)) ;;
        2) skipped=$((skipped + 1)) ;;
        *) warned=$((warned + 1)) ;;
    esac

    if ((warned > 0)); then
        add_summary "API key setup" "warn" "$configured added, $skipped skipped, $warned need manual setup"
    elif ((configured > 0)); then
        add_summary "API key setup" "ok" "$configured key entry or entries added; existing content preserved"
    else
        add_summary "API key setup" "skipped" "no absent key entries were added"
    fi
}

setup_ollama_model() {
    local list_output=""
    local model_count=0
    local model_name=""
    current_step="Ollama model setup"

    if [[ "$ollama_installed" != true ]]; then
        add_summary "Ollama model" "skipped" "ollama CLI unavailable"
        return
    fi

    if ! list_output="$(ollama list 2>&1)"; then
        printf '%s\n' "$list_output"
        printf 'warning: ollama list failed; start Ollama and run it manually\n' >&2
        add_summary "Ollama model" "warn" "ollama list failed; run it manually after starting Ollama"
        return
    fi
    printf '\nInstalled Ollama models:\n%s\n' "$list_output"
    ollama_reachable=true
    model_count="$(printf '%s\n' "$list_output" | awk 'NR > 1 && NF { count += 1 } END { print count + 0 }')"
    if ((model_count > 0)); then
        add_summary "Ollama model" "skipped" "$model_count model(s) already installed"
        return
    fi

    if [[ "$mode" == "yes" ]]; then
        printf 'skipped: no Ollama model is installed and --yes will not choose one; run ollama pull "<your-model>"\n'
        add_summary "Ollama model" "skipped" "no model selected; run ollama pull <your-model>"
        return
    fi
    if ! confirm "No Ollama models were found. Pull one now?"; then
        add_summary "Ollama model" "skipped" "no model selected"
        return
    fi
    printf 'Model name (no default): '
    if ! IFS= read -r model_name; then
        model_name=""
    fi
    if [[ -z "$model_name" ]]; then
        printf 'warning: no model name entered; run ollama pull "<your-model>" manually\n' >&2
        add_summary "Ollama model" "warn" "model name was empty; manual pull required"
        return
    fi
    if ! ollama pull "$model_name"; then
        printf 'warning: Ollama pull failed; run ollama pull "%s" manually\n' "$model_name" >&2
        add_summary "Ollama model" "warn" "pull failed; retry manually"
        return
    fi
    add_summary "Ollama model" "ok" "requested model pulled"
}

setup_interaction_probe() {
    local probe_output=""
    current_step="interaction probe setup"
    if [[ "$node_ready" != true ]]; then
        add_summary "Interaction probe" "skipped" "node and npm are required"
        return
    fi
    if ! confirm "Run 'commandagent --setup-interaction-probe'?"; then
        add_summary "Interaction probe" "skipped" "user declined probe setup"
        return
    fi
    if ! probe_output="$("$commandagent_bin" --setup-interaction-probe 2>&1)"; then
        printf '%s\n' "$probe_output"
        printf 'warning: interaction probe setup failed; run commandagent --setup-interaction-probe manually\n' >&2
        add_summary "Interaction probe" "warn" "setup failed; retry manually"
        return
    fi
    printf '%s\n' "$probe_output"
    if [[ "$probe_output" == *"existing playwright"* ]]; then
        add_summary "Interaction probe" "skipped" "existing managed Playwright installation reused"
    else
        add_summary "Interaction probe" "ok" "managed Playwright probe prepared"
    fi
}

smoke_version() {
    local version_output=""
    current_step="CommandAgent version smoke test"
    if ! version_output="$("$commandagent_bin" --version 2>&1)"; then
        fail "commandagent --version failed; run '$commandagent_bin --version' manually and inspect the error"
    fi
    printf '\nCommandAgent version:\n%s\n' "$version_output"
    add_summary "Version smoke test" "ok" "$version_output"
}

smoke_model_probe() {
    local probe_output=""
    current_step="model probe smoke test"
    if [[ "$ollama_reachable" != true ]]; then
        add_summary "Model probe" "skipped" "$ollama_url was not reachable"
        return
    fi
    printf '\nThe model probe can take several minutes and sends test prompts to the configured model.\n'
    if ! confirm "Run 'commandagent --model-probe' now?"; then
        add_summary "Model probe" "skipped" "long-running smoke test declined"
        return
    fi
    if ! probe_output="$("$commandagent_bin" --model-probe 2>&1)"; then
        printf '%s\n' "$probe_output"
        printf 'warning: model probe failed; verify model configuration and run commandagent --model-probe manually\n' >&2
        add_summary "Model probe" "warn" "probe failed; retry manually after checking model configuration"
        return
    fi
    printf '%s\n' "$probe_output"
    add_summary "Model probe" "ok" "model smoke test completed"
}

prepare_extension_root() {
    local canonical=""
    if [[ -z "$extension_root" && "$write_config" == true ]]; then
        extension_root="$repo_root/../commandagent-extensions"
    fi
    if [[ -z "$extension_root" ]]; then
        return
    fi
    current_step="extension root setup"
    if [[ "$extension_root" == *$'\n'* || "$extension_root" == *'"'* ]]; then
        fail "--extension-root cannot contain a newline or double quote"
    fi
    mkdir -p -- "$extension_root/packs" "$extension_root/profiles"
    canonical="$(CDPATH='' cd -- "$extension_root" && pwd -P)"
    case "$canonical/" in
        "$repo_root/"* ) fail "extension root must be disjoint from repository root" ;;
    esac
    case "$repo_root/" in
        "$canonical/"* ) fail "extension root must be disjoint from repository root" ;;
    esac
    extension_root=$canonical
    chmod 700 "$extension_root" "$extension_root/packs" "$extension_root/profiles"
    if [[ ! -e "$extension_root/journal.jsonl" ]]; then
        (umask 177 && : >"$extension_root/journal.jsonl")
    fi
    printf 'ok: prepared private extension root at %s\n' "$extension_root"
    add_summary "Extension root" "ok" "packs/, profiles/, and journal.jsonl prepared"
}

write_config_template() {
    local config_dir="$repo_root/.commandagent"
    local config_file="$config_dir/config.toml"
    local candidate=""
    if [[ "$write_config" != true ]]; then
        return
    fi
    current_step="config template"
    mkdir -p -- "$config_dir"
    candidate="$(mktemp "$config_dir/config.toml.candidate.XXXXXX")"
    chmod 600 "$candidate"
    {
        printf '# CommandAgent workspace configuration. Customize the example preset before use.\n'
        printf 'extension_root = "%s"\n\n' "$extension_root"
        printf '[preset.nextjs_acme_cagentpack]\n'
        printf 'profile = "nextjs"\n'
        printf 'provider = "ollama"\n'
        printf 'model = "qwen3.6:27b-coding-nvfp4"\n'
        printf 'planner_provider = "ollama"\n'
        printf 'planner_model = "qwen3.6:27b-coding-nvfp4"\n'
        printf 'plan_preset = "profile"\n'
        printf 'pack = "nextjs-acme@1.0.0"\n'
    } >"$candidate"
    if [[ -e "$config_file" ]]; then
        printf 'skipped: %s already exists; review this proposed diff:\n' "$config_file"
        diff -u --label "$config_file" --label "proposed config" "$config_file" "$candidate" || true
        rm -f -- "$candidate"
        add_summary "Config" "skipped" "existing config preserved; proposed diff shown"
        return
    fi
    mv -- "$candidate" "$config_file"
    chmod 600 "$config_file"
    printf 'ok: wrote config template at %s\n' "$config_file"
    add_summary "Config" "ok" "workspace config template created"
}

write_gui_token() {
    local token_dir=""
    local temporary=""
    if [[ -z "$gui_token_file" ]]; then
        return
    fi
    current_step="GUI Trial token setup"
    if [[ "$mode" == "yes" ]]; then
        printf 'skipped: --yes never writes GUI Trial tokens; rerun without --yes\n'
        gui_token_file=""
        add_summary "GUI token" "skipped" "non-interactive mode never writes secrets"
        return
    fi
    if [[ -e "$gui_token_file" ]]; then
        printf 'skipped: GUI Trial token file already exists at %s; it was not changed\n' "$gui_token_file"
        add_summary "GUI token" "skipped" "existing token file preserved"
        return
    fi
    command -v openssl >/dev/null 2>&1 \
        || fail "openssl is required to generate --gui-token-file safely"
    token_dir=${gui_token_file%/*}
    if [[ "$token_dir" == "$gui_token_file" ]]; then
        token_dir="."
    fi
    mkdir -p -- "$token_dir"
    temporary="${gui_token_file}.tmp.$$"
    (umask 177 && openssl rand -hex 32 >"$temporary")
    chmod 600 "$temporary"
    mv -- "$temporary" "$gui_token_file"
    printf 'ok: wrote private GUI Trial token to %s (value hidden)\n' "$gui_token_file"
    add_summary "GUI token" "ok" "private token file created; value hidden"
}

setup_gui() {
    local gui_server_bin="$repo_root/target/debug/gui_server"
    local auth_mode="off"
    local doctor_output=""
    if [[ "$gui_enabled" != true ]]; then
        return
    fi
    current_step="GUI dependency installation and build"
    (cd -- "$repo_root/gui" && npm ci --include=dev)
    (cd -- "$repo_root/gui" && GUI_BASE_PATH="$gui_base_path" npm run build)
    (cd -- "$repo_root" && cargo build --features gui --bin gui_server)
    [[ -x "$gui_server_bin" ]] || fail "GUI build completed without $gui_server_bin"
    if [[ -n "$gui_token_file" && -f "$gui_token_file" ]]; then
        auth_mode="on"
    fi
    printf '\nGUI start command (initializes private roots and runs preflight):\n'
    print_gui_command "$gui_server_bin" "$auth_mode"
    add_summary "GUI" "ok" "built for base path $gui_base_path; --init startup command shown"

    current_step="CommandAgent doctor"
    if doctor_output="$(cd -- "$repo_root" && "$commandagent_bin" --doctor --json 2>&1)"; then
        printf '\nCommandAgent doctor:\n%s\n' "$doctor_output"
        add_summary "Doctor" "ok" "commandagent --doctor --json passed"
    else
        printf '\nCommandAgent doctor:\n%s\n' "$doctor_output"
        printf 'warning: commandagent --doctor --json reported a problem; inspect the report above\n' >&2
        add_summary "Doctor" "warn" "doctor reported a configuration or environment problem"
    fi
}

print_gui_command() {
    local gui_server_bin=$1
    local auth_mode=$2
    if [[ "$auth_mode" == "on" ]]; then
        printf "GUI_TRIAL_TOKEN=\"\$(<%q)\" " "$gui_token_file"
    fi
    printf '%q --init --base-path %q --static-dir %q --repository-root %q ' \
        "$gui_server_bin" "$gui_base_path" "$repo_root/gui/out" "$repo_root"
    if [[ -n "$extension_root" ]]; then
        printf '%s %q ' "--extension-root" "$extension_root"
    fi
    printf '%s %q' "--trial-token-auth" "$auth_mode"
    printf '\n'
}

check_prerequisites
if [[ "$check_only" == true ]]; then
    add_summary "Changes" "skipped" "--check-only made no changes"
    print_summary
    exit 0
fi

install_or_build
setup_shell_completion
configure_api_keys
setup_ollama_model
setup_interaction_probe
smoke_version
smoke_model_probe
prepare_extension_root
write_config_template
write_gui_token
setup_gui
print_summary
