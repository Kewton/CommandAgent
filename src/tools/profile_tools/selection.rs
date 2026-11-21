//! The versioned opt-in declaration, and which declarations this run selected.
//!
//! Only the effective profile and the effective pack selection are consulted;
//! other loaded profiles and packs never contribute. A profile's declaration is
//! bound to the exact-byte hash of its manifest. A pack's is read from the
//! selected, verified bytes: nothing another decode saw can change the answer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;

use super::{ProfileToolRefusal, RefusalKind};
use crate::planner::artifact_contract_checks::protocol;
use crate::planner::artifact_contract_checks::read_regular_file;
use crate::planner::pack::{LoadedPack, PackProfile, catalog, load_declared_directory};
use crate::planner::profile_manifest::source::{EXTENSION_MANIFEST_FILE, exact_byte_hash};
use crate::planner::structured_stages::contract::StageSetDeclaration;
use crate::tools::registry::ToolRegistry;

pub const DECLARATION_SCHEMA: &str = "profile-tool.declaration/v1";
/// The same declaration with `stages`. A tool in a v1 declaration keeps its one submission.
pub const STAGE_DECLARATION_SCHEMA: &str = "profile-tool.declaration/v2";

// The selection variables and pin file `planner::pack::runtime` owns. Its
// verification is private to that module, so `verify_selected_pack` repeats it
// over the same public loader and is held to it by tests.
const PACK_DIRECTORY_ENV: &str = "COMMANDAGENT_PACK_DIRECTORY";
const PACK_ID_ENV: &str = "COMMANDAGENT_PACK_ID";
const PACK_VERSION_ENV: &str = "COMMANDAGENT_PACK_VERSION";
const PACK_HASH_ENV: &str = "COMMANDAGENT_PACK_HASH";
const PACK_PIN_FILE: &str = "pack.sha256";

/// What a profile or pack may say. Unknown fields are refused, so an executable,
/// argv, module URL, expected hash, or permission cannot be expressed here.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileToolDeclaration {
    pub schema_version: String,
    /// The builtin tools this declaration keeps. It can only remove tools; the
    /// existing mode, allow policy, and path rules still apply to the rest.
    pub builtin_tools: Vec<String>,
    #[serde(default)]
    pub tools: Vec<DeclaredTool>,
    /// Obligations over this declaration's tools (G4). Opaque IDs, added checks and dependencies,
    /// and limits it may only narrow.
    #[serde(default)]
    pub(crate) stages: Option<Box<StageSetDeclaration>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredTool {
    pub id: String,
    pub effect: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub max_input_bytes: Option<usize>,
    #[serde(default)]
    pub max_output_bytes: Option<usize>,
}

impl ProfileToolDeclaration {
    /// The checks that need no registry, run when the manifest or pack decodes.
    pub fn validate(&self) -> Result<(), String> {
        let expected = match &self.stages {
            Some(_) => STAGE_DECLARATION_SCHEMA,
            None => DECLARATION_SCHEMA,
        };
        if self.schema_version != expected {
            return Err(format!("schema_version must be `{expected}`"));
        }
        if self.builtin_tools.is_empty() && self.tools.is_empty() {
            return Err("a declaration must keep a builtin tool or name a tool".to_string());
        }
        let default_registry = ToolRegistry::default();
        let builtins = default_registry
            .specs()
            .iter()
            .map(|spec| spec.function.name.as_str())
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for name in &self.builtin_tools {
            if !builtins.contains(name.as_str()) {
                return Err(format!("builtin_tools names unknown tool `{name}`"));
            }
            if !seen.insert(name.as_str()) {
                return Err(format!("builtin_tools repeats `{name}`"));
            }
        }
        let mut ids = BTreeSet::new();
        for tool in &self.tools {
            protocol::validate_slug(&tool.id)
                .map_err(|reason| format!("tool id `{}` {reason}", tool.id))?;
            if !ids.insert(tool.id.as_str()) {
                return Err(format!("tool `{}` is declared twice", tool.id));
            }
            let mut requires = BTreeSet::new();
            if let Some(feature) = tool.requires.iter().find(|item| !requires.insert(*item)) {
                return Err(format!("tool `{}` repeats `{feature}`", tool.id));
            }
        }
        Ok(())
    }
}

/// The merged declarations of this run's profile and pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedTools {
    /// The intersection across every declaring source: no source can add back a
    /// builtin another source removed.
    pub(super) builtin_tools: BTreeSet<String>,
    /// Sorted by opaque ID; this is the stable publication order's input.
    pub(super) tools: BTreeMap<String, DeclaredTool>,
    /// From the one source that declares them.
    pub(super) stages: Option<Box<StageSetDeclaration>>,
}

static MANIFEST_DECLARATIONS: Mutex<BTreeMap<String, ProfileToolDeclaration>> =
    Mutex::new(BTreeMap::new());
/// The value the CLI decoded for the selection its environment guard installed,
/// present only while that guard lives.
static INSTALLED_DECLARATION: Mutex<Option<(PackSelection, ResolvedDeclaration)>> =
    Mutex::new(None);

/// Remember a decoded manifest's declaration under the hash extension
/// registration records for the same bytes.
pub(crate) fn record_manifest_declaration(manifest: &str, declaration: ProfileToolDeclaration) {
    let hash = exact_byte_hash(EXTENSION_MANIFEST_FILE, manifest.as_bytes());
    MANIFEST_DECLARATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(hash, declaration);
}

/// What the CLI read from the bytes it verified for a selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedDeclaration {
    pub(crate) pack_profile: PackProfile,
    pub(crate) tools: Option<ProfileToolDeclaration>,
}

/// Holds a selection's value for admission; dropping it restores what was there.
pub(crate) struct InstalledDeclaration {
    previous: Option<(PackSelection, ResolvedDeclaration)>,
}

pub(crate) fn install_declaration(
    selection: PackSelection,
    declared: Option<ResolvedDeclaration>,
) -> InstalledDeclaration {
    let entry = declared.map(|declared| (selection, declared));
    InstalledDeclaration {
        previous: std::mem::replace(&mut *installed_declaration_slot(), entry),
    }
}

impl Drop for InstalledDeclaration {
    fn drop(&mut self) {
        *installed_declaration_slot() = self.previous.take();
    }
}

fn installed_declaration_slot()
-> std::sync::MutexGuard<'static, Option<(PackSelection, ResolvedDeclaration)>> {
    INSTALLED_DECLARATION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The installed value, only for the exact selection it was decoded for.
fn installed_declaration(selection: &PackSelection) -> Option<ResolvedDeclaration> {
    installed_declaration_slot()
        .as_ref()
        .filter(|(installed, _)| installed == selection)
        .map(|(_, declared)| declared.clone())
}

/// The effective profile's and pack's declarations, or `None` when neither
/// declares anything. With no pack selected, the `None` path reads no file.
pub(super) fn selected(
    profile: &str,
    pack: PackSource,
) -> Result<Option<SelectedTools>, ProfileToolRefusal> {
    let mut sources = Vec::new();
    if let Some(declaration) = profile_declaration(profile) {
        sources.push(declaration);
    }
    let gates_ran = matches!(pack, PackSource::Environment);
    let (selection, resolved) = match pack {
        PackSource::Environment | PackSource::InheritedBeforeGates => {
            let selection = pack_selection_from_environment();
            let resolved = selection.as_ref().and_then(installed_declaration);
            (selection, resolved)
        }
        PackSource::Selected {
            selection,
            declared,
        } => (Some(selection), declared),
        PackSource::None => (None, None),
    };
    if let Some(selection) = selection
        && let Some(declaration) =
            pack_declaration(profile, &selection, resolved.as_ref(), gates_ran)?
    {
        sources.push(declaration);
    }
    if sources.is_empty() {
        return Ok(None);
    }
    merge(&sources).map(Some)
}

/// Whether this run's profile or installed pack declares stages, from what is already in memory.
/// Final acceptance asks this on every run, so it may not read a file.
pub(crate) fn stages_declared_without_a_read(profile: &str) -> bool {
    let by_profile = profile_declaration(profile).is_some_and(|declared| declared.stages.is_some());
    by_profile
        || pack_selection_from_environment()
            .as_ref()
            .and_then(installed_declaration)
            .and_then(|resolved| resolved.tools)
            .is_some_and(|declared| declared.stages.is_some())
}

/// This run's pack selection, for a final gate that may have stages to ask about. `None` is a run
/// with none, known from the environment and memory: no profile declaration, and no pack or a pack
/// whose decoded value has no stage. A selection with no decoded value is not known either way.
pub(crate) fn selection_that_may_hold_stages(profile: &str) -> Option<Option<PackSelection>> {
    let pack = pack_selection_from_environment();
    let installed = pack.as_ref().and_then(installed_declaration);
    let unknown = pack.is_some() && installed.is_none();
    let may_hold = unknown || stages_declared_without_a_read(profile);
    may_hold.then_some(pack)
}

/// The selection only an admission can read, for it to say what it found. `None` otherwise.
pub(crate) fn selection_left_to_admission() -> Option<PackSelection> {
    pack_selection_from_environment().filter(|pack| installed_declaration(pack).is_none())
}

/// The read a run makes only when no session read the selection for it. A selection that cannot be
/// read is an `Err`, as in a session: it must not turn a declaration off.
pub(crate) fn selected_stages_by_reading(profile: &str) -> Result<bool, ProfileToolRefusal> {
    selected(profile, PackSource::Environment)
        .map(|selected| selected.is_some_and(|selected| selected.stages.is_some()))
}

#[cfg(test)]
pub(super) fn manifest_declaration_for_test(profile: &str) -> Option<ProfileToolDeclaration> {
    profile_declaration(profile)
}

fn profile_declaration(profile: &str) -> Option<ProfileToolDeclaration> {
    let extension = crate::planner::extension_profiles::find(profile)?;
    MANIFEST_DECLARATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(extension.manifest_hash)
        .cloned()
}

/// One selected pack: the variables the pack runtime reads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackSelection {
    pub(crate) directory: PathBuf,
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) hash: String,
}

/// Where this admission finds the selected pack. Before the CLI installs its
/// selection in the environment, it passes the selection directly.
#[derive(Debug, Clone)]
pub(super) enum PackSource {
    /// A session or planner call: the pack gates have run, so a selection that no
    /// longer verifies is refused here rather than leaving the tools unrestricted.
    Environment,
    /// The CLI gate, for a selection it inherited: a failure of a pack not known
    /// to declare stays the pack gates' refusal, at their place in the run.
    InheritedBeforeGates,
    /// The CLI gate, for a selection it resolved, with the same rule. `declared`
    /// is what it decoded; without one, the selection is read.
    Selected {
        selection: PackSelection,
        declared: Option<ResolvedDeclaration>,
    },
    None,
}

pub(crate) fn pack_selection_from_environment() -> Option<PackSelection> {
    let value = |name: &str| std::env::var_os(name)?.into_string().ok();
    Some(PackSelection {
        directory: PathBuf::from(value(PACK_DIRECTORY_ENV)?),
        id: value(PACK_ID_ENV)?,
        version: value(PACK_VERSION_ENV)?,
        hash: value(PACK_HASH_ENV)?,
    })
}

/// What the selected, verified bytes declare. A value the CLI decoded for this pack's
/// profile answers without a read; another profile is judged from the bytes. Before
/// the pack gates, a pack that does not decode or does not declare is not known to opt
/// in, so its failure stays their refusal to make. After them it is refused: an
/// unreadable selection must not turn a declared allowlist off.
fn pack_declaration(
    profile: &str,
    selection: &PackSelection,
    resolved: Option<&ResolvedDeclaration>,
    gates_ran: bool,
) -> Result<Option<ProfileToolDeclaration>, ProfileToolRefusal> {
    let expected = crate::planner::profile_descriptor::pack_profile_for_name(profile);
    if let Some(resolved) = resolved
        && expected.as_ref() == Some(&resolved.pack_profile)
    {
        return Ok(resolved.tools.clone());
    }
    let invalid =
        |reason: String| ProfileToolRefusal::new(RefusalKind::SelectedPackInvalid, reason);
    let declared = match load_declared_directory(&selection.directory) {
        Ok(declared) => declared,
        Err(error) if gates_ran => return Err(invalid(format!("load selected pack: {error}"))),
        Err(_) => return Ok(None),
    };
    let pack = declared.pack;
    let verified = verify_selected_pack(selection, &pack);
    let Some(declaration) = declared.profile_tools else {
        return match verified {
            Ok(()) => Ok(None),
            Err(reason) if gates_ran => Err(invalid(reason)),
            Err(_) => Ok(None),
        };
    };
    verified.map_err(invalid)?;
    if expected.as_ref() != Some(&pack.identity.profile) {
        return Err(ProfileToolRefusal::new(
            RefusalKind::SelectedPackInvalid,
            format!(
                "selected pack `{}` is for profile `{}`, not `{profile}`",
                selection.id, pack.identity.profile
            ),
        ));
    }
    Ok(Some(declaration))
}

/// Retirement, identity, exact-byte hash, and pin, as the pack runtime verifies
/// a selection, without following a symlink for the pin.
fn verify_selected_pack(selection: &PackSelection, pack: &LoadedPack) -> Result<(), String> {
    let directory: &Path = &selection.directory;
    if catalog::is_retired(directory) {
        return Err(format!(
            "selected pack {}@{} is retired and cannot run",
            selection.id, selection.version
        ));
    }
    if pack.id() != selection.id
        || pack.identity.version != selection.version
        || pack.hash != selection.hash
    {
        return Err(format!(
            "selected pack identity/hash mismatch: expected {}@{}",
            selection.id, selection.version
        ));
    }
    let pin = read_regular_file(&directory.join(PACK_PIN_FILE), 4096)
        .map_err(|reason| format!("read selected pack hash pin: {reason}"))?;
    if String::from_utf8_lossy(&pin).trim() != selection.hash {
        return Err("selected pack hash does not match pack.sha256".to_string());
    }
    Ok(())
}

fn merge(sources: &[ProfileToolDeclaration]) -> Result<SelectedTools, ProfileToolRefusal> {
    let mut builtin_tools: Option<BTreeSet<String>> = None;
    let mut tools = BTreeMap::new();
    let mut stages = None;
    for source in sources {
        if source.stages.is_some() && stages.replace(source.stages.clone()).is_some() {
            return Err(ProfileToolRefusal::new(
                RefusalKind::ConflictingDeclaration,
                "stages are declared by both profile and pack",
            ));
        }
        let kept = source
            .builtin_tools
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        builtin_tools = Some(match builtin_tools {
            Some(existing) => existing.intersection(&kept).cloned().collect(),
            None => kept,
        });
        for tool in &source.tools {
            if tools.insert(tool.id.clone(), tool.clone()).is_some() {
                return Err(ProfileToolRefusal::new(
                    RefusalKind::ConflictingDeclaration,
                    format!("tool `{}` is declared by both profile and pack", tool.id),
                ));
            }
        }
    }
    Ok(SelectedTools {
        builtin_tools: builtin_tools.unwrap_or_default(),
        tools,
        stages: stages.flatten(),
    })
}

#[cfg(test)]
mod stage_declaration_tests {
    use serde_json::json;

    use super::{ProfileToolDeclaration, merge};

    fn declaration(schema: &str, stages: bool) -> ProfileToolDeclaration {
        let mut value = json!({"schema_version": schema, "builtin_tools": ["Read"]});
        if stages {
            value["stages"] = json!({
                "stages": [{"id": "only", "targets": [{"id": "main", "tool_id": "a-tool"}]}],
                "limits": {"attempts_per_target": 2},
            });
        }
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn stages_come_with_their_own_version_and_a_v1_declaration_stays_as_it_was() {
        let v1 = "profile-tool.declaration/v1";
        let v2 = "profile-tool.declaration/v2";
        assert!(declaration(v1, false).validate().is_ok());
        assert!(declaration(v2, true).validate().is_ok());
        assert!(declaration(v1, true).validate().is_err());
        assert!(declaration(v2, false).validate().is_err());
    }

    #[test]
    fn anything_else_a_stage_declaration_says_is_refused() {
        for extra in [
            json!({"stages": [], "executable": "/bin/sh"}),
            json!({"stages": [{"id": "s", "targets": [], "command": "x"}]}),
            json!({"stages": [{"id": "s", "targets": [{"id": "t", "tool_id": "a", "path": "p"}]}]}),
            json!({"stages": [], "limits": {"attempts": 9}}),
        ] {
            let value = json!({
                "schema_version": "profile-tool.declaration/v2",
                "builtin_tools": ["Read"],
                "stages": extra,
            });
            assert!(
                serde_json::from_value::<ProfileToolDeclaration>(value).is_err(),
                "{extra}"
            );
        }
    }

    #[test]
    fn one_source_declares_stages() {
        let v2 = "profile-tool.declaration/v2";
        let selected = merge(&[
            declaration(v2, true),
            declaration("profile-tool.declaration/v1", false),
        ])
        .unwrap();
        assert!(selected.stages.is_some());
        assert!(merge(&[declaration(v2, true), declaration(v2, true)]).is_err());
    }
}
