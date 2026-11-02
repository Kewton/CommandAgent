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
use crate::planner::pack::{LoadedPack, catalog, load_declared_directory};
use crate::planner::profile_manifest::source::{EXTENSION_MANIFEST_FILE, exact_byte_hash};
use crate::tools::registry::ToolRegistry;

pub const DECLARATION_SCHEMA: &str = "profile-tool.declaration/v1";

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
        if self.schema_version != DECLARATION_SCHEMA {
            return Err(format!("schema_version must be `{DECLARATION_SCHEMA}`"));
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
}

static MANIFEST_DECLARATIONS: Mutex<BTreeMap<String, ProfileToolDeclaration>> =
    Mutex::new(BTreeMap::new());
/// Exact selections, hash included, whose verified bytes declare nothing, so the
/// next admission of one reads nothing more. No other decode adds to this.
static VERIFIED_UNDECLARED: Mutex<BTreeSet<PackSelection>> = Mutex::new(BTreeSet::new());

/// Remember a decoded manifest's declaration under the hash extension
/// registration records for the same bytes.
pub(crate) fn record_manifest_declaration(manifest: &str, declaration: ProfileToolDeclaration) {
    let hash = exact_byte_hash(EXTENSION_MANIFEST_FILE, manifest.as_bytes());
    MANIFEST_DECLARATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(hash, declaration);
}

fn verified_undeclared() -> std::sync::MutexGuard<'static, BTreeSet<PackSelection>> {
    VERIFIED_UNDECLARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
    let selection = match pack {
        PackSource::Environment | PackSource::InheritedBeforeGates => {
            pack_selection_from_environment()
        }
        PackSource::Selected(selection) => Some(selection),
        PackSource::None => None,
    };
    if let Some(selection) = selection
        && let Some(declaration) = pack_declaration(profile, &selection, gates_ran)?
    {
        sources.push(declaration);
    }
    if sources.is_empty() {
        return Ok(None);
    }
    merge(&sources).map(Some)
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
pub(super) struct PackSelection {
    pub(super) directory: PathBuf,
    pub(super) id: String,
    pub(super) version: String,
    pub(super) hash: String,
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
    /// The CLI gate, for a selection it resolved, with the same rule.
    Selected(PackSelection),
    None,
}

fn pack_selection_from_environment() -> Option<PackSelection> {
    let value = |name: &str| std::env::var_os(name)?.into_string().ok();
    Some(PackSelection {
        directory: PathBuf::from(value(PACK_DIRECTORY_ENV)?),
        id: value(PACK_ID_ENV)?,
        version: value(PACK_VERSION_ENV)?,
        hash: value(PACK_HASH_ENV)?,
    })
}

/// What the selected, verified bytes declare. Before the pack gates, a pack that
/// does not decode or does not declare is not known to opt in, so its failure stays
/// their refusal to make. After them it is refused: an unreadable selection must
/// not turn a declared allowlist off.
fn pack_declaration(
    profile: &str,
    selection: &PackSelection,
    gates_ran: bool,
) -> Result<Option<ProfileToolDeclaration>, ProfileToolRefusal> {
    if verified_undeclared().contains(selection) {
        return Ok(None);
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
            Ok(()) => {
                verified_undeclared().insert(selection.clone());
                Ok(None)
            }
            Err(reason) if gates_ran => Err(invalid(reason)),
            Err(_) => Ok(None),
        };
    };
    verified.map_err(invalid)?;
    let expected = crate::planner::profile_descriptor::pack_profile_for_name(profile);
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
    for source in sources {
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
    })
}
