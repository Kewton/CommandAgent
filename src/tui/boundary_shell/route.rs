use std::collections::BTreeSet;
use std::path::Path;

use crate::planner::adjudication::contract::{IntentId, intent_contract};
use crate::planner::profile::{ProfileId, ProfileRuntimeRegistry};
use crate::planner::profile_manifest::ManifestStatus;

use super::band_catalog::{BandValue, value_for};
use super::family_catalog::{TASK_FAMILY_CATALOG, TaskFamilyId};

const MAX_INVENTORY_ENTRIES: usize = 256;
const MAX_INVENTORY_DEPTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteBasis {
    pub rule: &'static str,
    pub observation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteCandidate {
    pub profile: ProfileId,
    pub intent: IntentId,
    pub family: TaskFamilyId,
    pub bases: Vec<RouteBasis>,
    pub contract_ref: &'static str,
}

impl RouteCandidate {
    pub fn band(&self) -> Option<&'static BandValue> {
        value_for(route_profile_name(&self.profile), self.intent, self.family)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeterministicResolution {
    Unique,
    Ambiguous,
    Unknown,
    ContradictoryExplicitBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicRouteResult {
    pub resolution: DeterministicResolution,
    pub candidates: Vec<RouteCandidate>,
    pub observations: Vec<RouteBasis>,
    pub inventory_omitted: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplicitRouteBinding {
    pub profile: Option<ProfileId>,
    pub intent: Option<IntentId>,
    pub family: Option<TaskFamilyId>,
}

pub struct RouteRequest<'a> {
    pub request: &'a str,
    pub workspace: &'a Path,
    pub explicit: ExplicitRouteBinding,
}

pub fn admitted_profiles() -> Vec<ProfileId> {
    ProfileRuntimeRegistry::registered()
        .filter(|profile| {
            crate::planner::profile_admission::status(route_profile_name(profile))
                == ManifestStatus::Admitted
        })
        .collect()
}

pub fn deterministic_route(request: RouteRequest<'_>) -> DeterministicRouteResult {
    let inventory = bounded_inventory(request.workspace);
    let mut observations = Vec::new();
    let mut profiles = BTreeSet::new();
    let mut intents = BTreeSet::new();
    let mut families = BTreeSet::new();

    if let Some(profile) = request.explicit.profile.clone() {
        observations.push(RouteBasis {
            rule: "explicit.profile",
            observation: profile.to_string(),
        });
        profiles.insert(route_profile_name(&profile).to_string());
    }
    if let Some(intent) = request.explicit.intent {
        observations.push(RouteBasis {
            rule: "explicit.intent",
            observation: intent.as_str().to_string(),
        });
        intents.insert(intent.as_str().to_string());
    }
    if let Some(family) = request.explicit.family {
        observations.push(RouteBasis {
            rule: "explicit.family",
            observation: family.to_string(),
        });
        families.insert(family);
    }

    observe_workspace(&inventory.paths, &mut profiles, &mut observations);
    observe_request(
        request.request,
        &mut profiles,
        &mut intents,
        &mut observations,
    );
    observe_families(
        request.request,
        &inventory.paths,
        &mut families,
        &mut observations,
    );

    let admitted = admitted_profiles()
        .into_iter()
        .map(|profile| route_profile_name(&profile).to_string())
        .collect::<BTreeSet<_>>();
    profiles.retain(|profile| admitted.contains(profile));

    let explicit_conflict =
        explicit_binding_conflicts(&request.explicit, &profiles, &intents, &families);
    if explicit_conflict {
        return DeterministicRouteResult {
            resolution: DeterministicResolution::ContradictoryExplicitBinding,
            candidates: Vec::new(),
            observations,
            inventory_omitted: inventory.omitted,
        };
    }

    let mut candidates = TASK_FAMILY_CATALOG
        .iter()
        .filter(|entry| {
            (profiles.is_empty() || profiles.contains(entry.profile))
                && (intents.is_empty() || intents.contains(entry.intent.as_str()))
                && (families.is_empty() || families.contains(&entry.id))
                && admitted.contains(entry.profile)
        })
        .map(|entry| RouteCandidate {
            profile: ProfileId::parse(entry.profile),
            intent: entry.intent,
            family: entry.id,
            bases: observations.clone(),
            contract_ref: contract_ref(entry.profile, entry.intent),
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (
            route_profile_name(&left.profile),
            left.intent.as_str(),
            left.family,
        )
            .cmp(&(
                route_profile_name(&right.profile),
                right.intent.as_str(),
                right.family,
            ))
    });

    let resolution = match candidates.len() {
        0 => DeterministicResolution::Unknown,
        1 => DeterministicResolution::Unique,
        _ => DeterministicResolution::Ambiguous,
    };
    DeterministicRouteResult {
        resolution,
        candidates,
        observations,
        inventory_omitted: inventory.omitted,
    }
}

fn contract_ref(profile: &str, intent: IntentId) -> &'static str {
    if intent != IntentId::Create {
        return intent_contract(intent.as_str())
            .expect("typed intent must have a contract")
            .contract_ref;
    }
    match profile {
        "nextjs" => "docs/nextjs-profile-contract.md",
        "data" => "docs/dev/data-profile-contract.md",
        "python-cli" => "docs/cli-profile-contract.md",
        "ingest" => "docs/ingest-profile-contract.md",
        _ => intent_contract("create").unwrap().contract_ref,
    }
}

fn route_profile_name(profile: &ProfileId) -> &str {
    match profile {
        ProfileId::Cli => "python-cli",
        _ => profile.as_str(),
    }
}

fn explicit_binding_conflicts(
    explicit: &ExplicitRouteBinding,
    observed_profiles: &BTreeSet<String>,
    observed_intents: &BTreeSet<String>,
    observed_families: &BTreeSet<TaskFamilyId>,
) -> bool {
    let profile_conflict = explicit.profile.as_ref().is_some_and(|profile| {
        observed_profiles.len() > 1 && observed_profiles.contains(route_profile_name(profile))
    });
    let intent_conflict = explicit.intent.is_some_and(|intent| {
        observed_intents.len() > 1 && observed_intents.contains(intent.as_str())
    });
    let family_conflict = explicit
        .family
        .is_some_and(|family| observed_families.len() > 1 && observed_families.contains(&family));
    profile_conflict || intent_conflict || family_conflict
}

fn observe_workspace(
    paths: &[String],
    profiles: &mut BTreeSet<String>,
    observations: &mut Vec<RouteBasis>,
) {
    let has = |needle: &str| paths.iter().any(|path| path == needle);
    let prefix = |needle: &str| paths.iter().any(|path| path.starts_with(needle));
    if prefix("data/snapshots/") {
        profiles.insert("ingest".to_string());
        observations.push(RouteBasis {
            rule: "workspace.snapshots",
            observation: "data/snapshots/".to_string(),
        });
    }
    if has("cli/main.py") {
        profiles.insert("python-cli".to_string());
        observations.push(RouteBasis {
            rule: "workspace.cli_main",
            observation: "cli/main.py".to_string(),
        });
    }
    if has("package.json") && (prefix("app/") || prefix("src/app/")) {
        profiles.insert("nextjs".to_string());
        observations.push(RouteBasis {
            rule: "workspace.nextjs_app_router",
            observation: "package.json + App Router".to_string(),
        });
    }
    if has("pipeline/main.py")
        && paths
            .iter()
            .any(|path| path.ends_with(".csv") || path.ends_with(".tsv"))
    {
        profiles.insert("data".to_string());
        observations.push(RouteBasis {
            rule: "workspace.tabular_pipeline",
            observation: "pipeline/main.py + tabular input".to_string(),
        });
    }
}

fn observe_request(
    request: &str,
    profiles: &mut BTreeSet<String>,
    intents: &mut BTreeSet<String>,
    observations: &mut Vec<RouteBasis>,
) {
    let lower = request.to_ascii_lowercase();
    let compact = lower.split_whitespace().collect::<String>();
    insert_on_tokens(
        &compact,
        &["作成", "作って", "生成", "build", "create", "make"],
        "create",
        "request.intent.create",
        intents,
        observations,
    );
    insert_on_tokens(
        &compact,
        &["修正", "直して", "repair", "fix"],
        "fix",
        "request.intent.fix",
        intents,
        observations,
    );
    insert_on_tokens(
        &compact,
        &[
            "調査",
            "診断",
            "原因",
            "investigate",
            "diagnose",
            "reproduce",
        ],
        "investigate",
        "request.intent.investigate",
        intents,
        observations,
    );
    insert_on_tokens(
        &compact,
        &["snapshot", "スナップショット", "htmlから", "イベント一覧"],
        "ingest",
        "request.profile.ingest",
        profiles,
        observations,
    );
    insert_on_tokens(
        &compact,
        &["cli", "コマンドライン", "--help"],
        "python-cli",
        "request.profile.cli",
        profiles,
        observations,
    );
    insert_on_tokens(
        &compact,
        &["next.js", "nextjs", "webアプリ", "クイズ", "ゲーム"],
        "nextjs",
        "request.profile.nextjs",
        profiles,
        observations,
    );
    insert_on_tokens(
        &compact,
        &["csv", "tsv", "集計", "移動平均", "前月比"],
        "data",
        "request.profile.data",
        profiles,
        observations,
    );
}

fn observe_families(
    request: &str,
    paths: &[String],
    families: &mut BTreeSet<TaskFamilyId>,
    observations: &mut Vec<RouteBasis>,
) {
    let lower = request.to_ascii_lowercase();
    let rules = [
        (TaskFamilyId::Quiz, &["quiz", "クイズ"][..]),
        (TaskFamilyId::Breakout, &["breakout", "ブロック崩し"][..]),
        (TaskFamilyId::Space, &["space", "invader", "インベーダ"][..]),
        (
            TaskFamilyId::Aggregation,
            &["月次×地域", "月次x地域", "地域別集計"][..],
        ),
        (TaskFamilyId::Timeseries, &["移動平均", "前月比"][..]),
        (
            TaskFamilyId::Stats,
            &["件数・合計・平均", "sum", "mean"][..],
        ),
        (
            TaskFamilyId::Filter,
            &["--pattern", "--count", "行を抽出"][..],
        ),
        (
            TaskFamilyId::CompileErrorFix,
            &["compile error", "build error", "コンパイル"][..],
        ),
        (
            TaskFamilyId::ContractHookFix,
            &["data-anvil", "contract hook", "restart hook"][..],
        ),
        (TaskFamilyId::Pipe, &["pipe", "パイプ"][..]),
        (TaskFamilyId::Schema, &["schema", "スキーマ"][..]),
    ];
    for (family, tokens) in rules {
        if tokens.iter().any(|token| lower.contains(token)) {
            families.insert(family);
            observations.push(RouteBasis {
                rule: "request.family",
                observation: family.to_string(),
            });
        }
    }
    if paths.iter().any(|path| {
        path.to_ascii_lowercase().contains("events-list")
            || path.to_ascii_lowercase().contains("list.html")
    }) || lower.contains("リスト構造")
        || lower.contains("一覧")
    {
        families.insert(TaskFamilyId::List);
        observations.push(RouteBasis {
            rule: "material.family.list",
            observation: "list-shaped snapshot".to_string(),
        });
    }
    if paths.iter().any(|path| {
        path.to_ascii_lowercase().contains("events-table")
            || path.to_ascii_lowercase().contains("table.html")
    }) || lower.contains("テーブル構造")
    {
        families.insert(TaskFamilyId::Table);
        observations.push(RouteBasis {
            rule: "material.family.table",
            observation: "table-shaped snapshot".to_string(),
        });
    }
}

fn insert_on_tokens(
    text: &str,
    tokens: &[&str],
    value: &str,
    rule: &'static str,
    output: &mut BTreeSet<String>,
    observations: &mut Vec<RouteBasis>,
) {
    if tokens.iter().any(|token| text.contains(token)) {
        output.insert(value.to_string());
        observations.push(RouteBasis {
            rule,
            observation: value.to_string(),
        });
    }
}

#[derive(Debug)]
struct Inventory {
    paths: Vec<String>,
    omitted: usize,
}

fn bounded_inventory(root: &Path) -> Inventory {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut paths = Vec::new();
    let mut omitted = 0;
    while let Some((directory, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = normalize_relative(relative);
            if ignored_path(&relative) || entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
                continue;
            }
            if paths.len() >= MAX_INVENTORY_ENTRIES {
                omitted += 1;
                continue;
            }
            paths.push(relative);
            if depth < MAX_INVENTORY_DEPTH && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push((path, depth + 1));
            }
        }
    }
    paths.sort();
    Inventory { paths, omitted }
}

fn normalize_relative(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn ignored_path(path: &str) -> bool {
    path.split('/').any(|component| {
        matches!(
            component,
            ".git" | ".anvil" | "node_modules" | "target" | ".next" | "__pycache__"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, relative: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "fixture").unwrap();
    }

    #[test]
    fn admitted_configuration_is_enumerated_from_runtime_admission_and_band_catalogs() {
        let profiles = admitted_profiles();
        for profile in [
            ProfileId::Nextjs,
            ProfileId::Data,
            ProfileId::PythonCli,
            ProfileId::Ingest,
        ] {
            assert!(profiles.contains(&profile));
            assert_eq!(
                ProfileRuntimeRegistry::resolve(&profile).profile_id(),
                profile
            );
        }
        assert!(
            TASK_FAMILY_CATALOG
                .iter()
                .all(|family| { value_for(family.profile, family.intent, family.id).is_some() })
        );
    }

    #[test]
    fn ingest_list_request_is_unique_only_with_rule_evidence() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "data/snapshots/events-list.html");
        let result = deterministic_route(RouteRequest {
            request: "HTMLのイベント一覧からレコードを作成してください",
            workspace: dir.path(),
            explicit: ExplicitRouteBinding::default(),
        });
        assert_eq!(result.resolution, DeterministicResolution::Unique);
        assert_eq!(result.candidates[0].profile, ProfileId::Ingest);
        assert_eq!(result.candidates[0].intent, IntentId::Create);
        assert_eq!(result.candidates[0].family, TaskFamilyId::List);
        assert!(!result.candidates[0].bases.is_empty());
        assert!(result.candidates[0].band().is_some());
    }

    #[test]
    fn false_deterministic_uniqueness_is_rejected_when_multiple_candidates_survive() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "data/snapshots/events-list.html");
        touch(dir.path(), "data/snapshots/events-table.html");
        let result = deterministic_route(RouteRequest {
            request: "HTMLスナップショットからイベントを作成してください",
            workspace: dir.path(),
            explicit: ExplicitRouteBinding::default(),
        });
        assert_eq!(result.resolution, DeterministicResolution::Ambiguous);
        assert_eq!(result.candidates.len(), 2);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.family)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([TaskFamilyId::List, TaskFamilyId::Table])
        );
    }

    #[test]
    fn symlinks_and_dependency_trees_do_not_create_route_evidence() {
        let dir = tempfile::tempdir().unwrap();
        touch(
            dir.path(),
            "node_modules/pkg/data/snapshots/events-list.html",
        );
        let result = deterministic_route(RouteRequest {
            request: "作成してください",
            workspace: dir.path(),
            explicit: ExplicitRouteBinding::default(),
        });
        assert_ne!(result.resolution, DeterministicResolution::Unique);
        assert!(
            result
                .observations
                .iter()
                .all(|basis| basis.rule != "workspace.snapshots")
        );
    }

    #[test]
    fn explicit_binding_that_conflicts_with_observed_route_stops_for_human_correction() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "data/snapshots/events-list.html");
        let result = deterministic_route(RouteRequest {
            request: "CLIを作成してください",
            workspace: dir.path(),
            explicit: ExplicitRouteBinding {
                profile: Some(ProfileId::PythonCli),
                intent: Some(IntentId::Create),
                family: Some(TaskFamilyId::Stats),
            },
        });
        assert_eq!(
            result.resolution,
            DeterministicResolution::ContradictoryExplicitBinding
        );
        assert!(result.candidates.is_empty());
    }
}
