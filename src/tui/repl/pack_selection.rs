//! The pack roots the REPL reads from, and the reasons a selection is refused.
//!
//! The CLI selects a `--pack` from the extension root first and the workspace
//! second (Issue #1057). The REPL read its Gate 1 and Gate 4 cards from the
//! workspace only, so a pack that `--pack` held from `--extension-root` made the
//! REPL fail. Every selection and every card here goes through the same locator
//! and the same rules as the CLI, and a pack the roots cannot read is refused
//! with a reason that never quotes the pack's bytes.

use std::path::{Path, PathBuf};

use anyhow::bail;

use crate::cli::Cli;
use crate::cli_pack::{ResolvedPack, SelectionSource};
use crate::config::Config;
use crate::planner::pack::catalog::{LocatedPack, PackLocator, PackSource};
use crate::planner::pack::{ConformanceError, PackError, PackProfile};
use crate::tui::boundary_shell::confirmation::PackSelection;

/// The pack roots one REPL reads from, plus the pack its `--pack` held at startup.
#[derive(Debug, Clone)]
pub struct ReplPackContext {
    locator: PackLocator,
    startup: Option<ResolvedPack>,
}

impl ReplPackContext {
    /// The roots alone, with no pack held at startup.
    pub fn new(workspace_root: impl Into<PathBuf>, extension_root: Option<PathBuf>) -> Self {
        Self::with_startup(workspace_root, extension_root, None)
    }

    /// The roots plus the pack a CLI `--pack` resolved, held for the REPL's life.
    pub(crate) fn with_startup(
        workspace_root: impl Into<PathBuf>,
        extension_root: Option<PathBuf>,
        startup: Option<ResolvedPack>,
    ) -> Self {
        Self {
            locator: PackLocator::with_extension_root(workspace_root, extension_root),
            startup,
        }
    }

    /// The context a CLI start hands over: the same roots its `--pack`
    /// resolution used, and the pack it resolved.
    pub(crate) fn from_cli(cli: &Cli, config: &Config, startup: Option<ResolvedPack>) -> Self {
        let extension_root = crate::cli_pack::effective_extension_root(cli, config)
            .ok()
            .flatten();
        Self::with_startup(config.workspace_root.clone(), extension_root, startup)
    }

    /// The context a REPL started without a CLI-resolved pack reads: the
    /// workspace and the configured top-level `extension_root`.
    pub(crate) fn from_config(config: &Config) -> Self {
        let extension_root = crate::config::configured_extension_root(&config.workspace_root)
            .ok()
            .flatten();
        Self::new(config.workspace_root.clone(), extension_root)
    }

    pub(crate) fn locator(&self) -> &PackLocator {
        &self.locator
    }

    /// Select one pack through the CLI's roots and rules: the extension root
    /// shadows the workspace even when its bytes are invalid, `local` is never
    /// admitted, and the profile and intent must match the route.
    pub fn select(
        &self,
        profile: &str,
        intent: &str,
        selector: &str,
    ) -> anyhow::Result<PackSelection> {
        crate::tui::boundary_shell::pack_catalog::select_with_locator(
            profile,
            intent,
            selector,
            &self.locator,
        )
    }

    pub(crate) fn validate(
        &self,
        profile: &str,
        intent: &str,
        selection: &PackSelection,
    ) -> anyhow::Result<()> {
        crate::tui::boundary_shell::pack_catalog::validate_selection_with_locator(
            profile,
            intent,
            selection,
            &self.locator,
        )
    }

    /// Select a pack for a Gate 1 card, returning the selection and the exact
    /// directory that answered for it. Freezing the directory is what later
    /// refuses a confirmation when the same selector has moved to another
    /// directory (for example the `packs/` compatible layout) or its bytes
    /// changed since the card was shown (Issue #1057).
    pub(crate) fn select_frozen(
        &self,
        profile: &str,
        intent: &str,
        selector: &str,
    ) -> anyhow::Result<(PackSelection, PathBuf)> {
        let selection = self.select(profile, intent, selector)?;
        let directory = self.frozen_directory(&selection)?;
        Ok((selection, directory))
    }

    /// The exact directory a pinned selection resolves to under these roots.
    pub(crate) fn frozen_directory(&self, selection: &PackSelection) -> anyhow::Result<PathBuf> {
        let PackSelection::Pinned {
            id,
            version,
            source,
            ..
        } = selection
        else {
            return Ok(PathBuf::new());
        };
        self.locator.locate(*source, id, version)
    }

    /// Revalidate the pack a Gate 1 card froze, before its confirmation is
    /// recorded. The exact directory that answered for the selector is checked
    /// again, never re-searched, so a selector that moved to another directory
    /// with the same bytes is refused. An admitted selection keeps the admitted
    /// catalog rule.
    pub(crate) fn revalidate_frozen(
        &self,
        profile: &str,
        intent: &str,
        selection: &PackSelection,
        directory: &Path,
    ) -> anyhow::Result<()> {
        let PackSelection::Pinned { hash, source, .. } = selection else {
            return Ok(());
        };
        if *source == PackSource::Admitted {
            return crate::tui::boundary_shell::pack_catalog::validate_selection(
                profile, intent, selection,
            );
        }
        let PackSelection::Pinned { id, version, .. } = selection else {
            return Ok(());
        };
        let located = self
            .locator
            .locate_pinned_at(*source, directory, id, version, Some(hash))?;
        crate::tui::boundary_shell::pack_catalog::validate_located_selection(
            profile, intent, selection, &located,
        )
    }

    pub(crate) fn observed_pin(&self, selection: &PackSelection) -> anyhow::Result<Option<String>> {
        crate::tui::boundary_shell::pack_catalog::observed_pin(&self.locator, selection)
    }

    /// The directory a confirmed selection dispatches from.
    pub(crate) fn directory(
        &self,
        source: PackSource,
        id: &str,
        version: &str,
    ) -> anyhow::Result<PathBuf> {
        self.locator.locate(source, id, version)
    }

    /// The bytes each selector resolves to under these roots, resolved the way a
    /// selection resolves it, for Gate 1's held-pack check before the planner is
    /// asked to route the request. `None` is a malformed selector.
    pub(crate) fn resolved_hash(&self, selector: &str) -> Option<String> {
        let (id, version) = split_selector(selector)?;
        self.locator
            .locate_pinned(id, version, None)
            .ok()
            .map(|located| located.hash)
    }

    /// Why a selector cannot be read under these roots, stated without bytes.
    pub(crate) fn selector_refusal(&self, selector: &str) -> String {
        let Some((id, version)) = split_selector(selector) else {
            return "the selector must pin one id@MAJOR.MINOR.PATCH".to_string();
        };
        match self.locator.locate_pinned(id, version, None) {
            Ok(_) => "the selector resolved".to_string(),
            Err(error) => refusal_reason(&error),
        }
    }

    /// The startup pack re-resolved through these roots, as the located bytes
    /// and the frozen selection Gate 1 would pin for it: what a request with no
    /// selector defaults to when `--pack` held a pack at startup.
    pub(crate) fn startup_selection(&self) -> anyhow::Result<Option<(LocatedPack, PackSelection)>> {
        let Some(startup) = self.startup.as_ref() else {
            return Ok(None);
        };
        let selection = self.revalidate_startup(startup)?;
        Ok(Some(selection))
    }

    fn revalidate_startup(
        &self,
        startup: &ResolvedPack,
    ) -> anyhow::Result<(LocatedPack, PackSelection)> {
        let source = pack_source(startup.source);
        let located = self.locator.locate_pinned_from(
            source,
            &startup.id,
            &startup.version,
            Some(&startup.hash),
        )?;
        // An admitted pack resolves from the repository root, so `Repository`
        // and `Admitted` are the same supply source here; only a move between
        // the extension root and the workspace is a real change.
        let same_root = match source {
            PackSource::Local => located.source == PackSource::Local,
            PackSource::Repository | PackSource::Admitted => {
                matches!(
                    located.source,
                    PackSource::Repository | PackSource::Admitted
                )
            }
        };
        if !same_root {
            bail!(
                "this REPL was started with `--pack {}@{}` ({}), but that pack now resolves from another supply source",
                startup.id,
                startup.version,
                startup.hash
            );
        }
        let point = located.point.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "pack `{}@{}` has no registered injection point",
                startup.id,
                startup.version
            )
        })?;
        let selection = PackSelection::Pinned {
            id: located.id.clone(),
            version: located.version.clone(),
            hash: located.hash.clone(),
            point,
            source: located.source,
        };
        Ok((located, selection))
    }

    /// The refusal of a request that defaults to the startup pack the roots can
    /// no longer read, or of a startup pack that is no longer selectable there.
    /// `None` when there is no startup pack or it revalidates.
    pub(crate) fn startup_refusal(&self) -> Option<String> {
        let startup = self.startup.as_ref()?;
        match self.revalidate_startup(startup) {
            Ok(_) => None,
            Err(error) => Some(startup_refusal(startup, &refusal_reason(&error))),
        }
    }
}

/// The selection source the REPL installs for a pack the CLI resolved there.
pub(crate) fn pack_source(source: SelectionSource) -> PackSource {
    match source {
        SelectionSource::ExtensionRoot => PackSource::Local,
        SelectionSource::Repository => PackSource::Repository,
    }
}

fn startup_refusal(startup: &ResolvedPack, reason: &str) -> String {
    format!(
        "this REPL was started with `--pack {}@{}` ({}), but that pack cannot be read under the REPL's pack roots: {reason}. Restart the REPL without `--pack` to select another pack.",
        startup.id, startup.version, startup.hash
    )
}

fn split_selector(selector: &str) -> Option<(&str, &str)> {
    let selector = selector.trim();
    let (id, version) = selector.split_once('@')?;
    if id.is_empty() || version.is_empty() || version.contains('@') {
        return None;
    }
    Some((id, version))
}

/// A refusal reason that names the failing class, never the pack's bytes.
///
/// The loader and the conformance checker put the offending content in their
/// own messages. A refusal reaches the terminal, the transcript, and the event
/// stream, so it states which check failed and nothing about the bytes.
pub(crate) fn refusal_reason(error: &anyhow::Error) -> String {
    for cause in error.chain() {
        if let Some(pack) = cause.downcast_ref::<PackError>() {
            return pack_error_reason(pack);
        }
        if let Some(conformance) = cause.downcast_ref::<ConformanceError>() {
            return conformance_error_reason(conformance);
        }
    }
    // Locator, pin, retirement, identity, and compatibility refusals are written
    // from paths and identities only; they never embed member bytes.
    error.to_string()
}

fn pack_error_reason(error: &PackError) -> String {
    match error {
        PackError::Empty => "the pack has neither assist.yaml nor eval.yaml".to_string(),
        PackError::NotRegularFile { .. } => "a pack member is not a regular file".to_string(),
        PackError::Io { .. } => "a pack member could not be read".to_string(),
        PackError::TooLarge { .. } => "a pack file exceeds the pack file size limit".to_string(),
        PackError::MaterialTooLarge { .. } => {
            "a pack material exceeds the material size limit".to_string()
        }
        PackError::MaterialsTooLarge => {
            "the pack materials exceed their total size limit".to_string()
        }
        PackError::UnsupportedMember { .. } => {
            "the pack contains an unsupported member".to_string()
        }
        PackError::InvalidMaterialName { .. } => {
            "the pack has an invalid material name".to_string()
        }
        PackError::MaterialNotUtf8 { .. } => "a pack material is not UTF-8".to_string(),
        PackError::Invalid { file, .. } => format!("`{file}` is invalid"),
        PackError::IdentityMismatch => {
            "assist.yaml and eval.yaml declare different pack identities".to_string()
        }
    }
}

fn conformance_error_reason(error: &ConformanceError) -> String {
    match error {
        ConformanceError::Load(inner) => pack_error_reason(inner),
        ConformanceError::FloorDefinition(_) => {
            "the registered contract floor is invalid".to_string()
        }
        ConformanceError::FloorViolation(_) => {
            "the pack does not conform to the registered contract floor".to_string()
        }
    }
}

/// Whether `located` can be read under this context for `profile` × `intent`,
/// with the rule the CLI applies, and the reason it cannot when it cannot.
pub(crate) fn compatibility_refusal(
    located: &LocatedPack,
    profile: &str,
    intent: &str,
) -> Option<String> {
    let Some(pack_profile) = PackProfile::parse(&located.profile) else {
        return Some(format!(
            "pack `{}@{}` names an unregistered profile",
            located.id, located.version
        ));
    };
    if !crate::planner::pack::catalog::profile_is_compatible(located.source, profile, pack_profile)
    {
        return Some(format!(
            "pack `{}@{}` is for profile `{}`, not `{profile}`",
            located.id, located.version, located.profile
        ));
    }
    if located.intent != intent {
        return Some(format!(
            "pack `{}@{}` is for intent `{}`, not `{intent}`",
            located.id, located.version, located.intent
        ));
    }
    if located.point.is_none() {
        return Some(format!(
            "pack `{}@{}` has no registered injection point",
            located.id, located.version
        ));
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;

    use super::*;

    /// A schema-valid assist member that is a renamed copy of the repository's
    /// `packs/<id>/1.0.0/assist.yaml`, with `marker` appended as a comment. The
    /// comment is part of the bytes, so a refusal that quotes them is
    /// observable, while the pack still conforms.
    pub(crate) fn assist(id: &str, version: &str, marker: &str) -> String {
        assist_from("cli-assist", id, version, marker)
    }

    pub(crate) fn assist_from(base_id: &str, id: &str, version: &str, marker: &str) -> String {
        let base = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("packs")
                .join(base_id)
                .join("1.0.0")
                .join("assist.yaml"),
        )
        .unwrap();
        let base = base
            .replacen(&format!("id: {base_id}"), &format!("id: {id}"), 1)
            .replacen("version: 1.0.0", &format!("version: {version}"), 1);
        format!("{base}\n# marker: {marker}\n")
    }

    /// Write a pinned pack at `root/<id>/<version>` in the direct layout, and
    /// return its exact-byte hash.
    pub(crate) fn write_pack(root: &Path, id: &str, version: &str, bytes: &str) -> String {
        let directory = root.join(id).join(version);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("assist.yaml"), bytes).unwrap();
        let hash = crate::planner::pack::exact_byte_hash(Some(bytes.as_bytes()), None);
        std::fs::write(directory.join("pack.sha256"), format!("{hash}\n")).unwrap();
        hash
    }

    #[test]
    fn a_loader_reason_never_quotes_the_pack_bytes() {
        let error = anyhow::Error::new(PackError::Invalid {
            file: "assist.yaml",
            reason: "invalid value: string \"PACK_CANARY_27\" at line 3".to_string(),
        });
        let reason = refusal_reason(&error);
        assert_eq!(reason, "`assist.yaml` is invalid");
        assert!(!reason.contains("PACK_CANARY_27"), "{reason}");
    }

    #[test]
    fn a_conformance_reason_never_quotes_the_pack_bytes() {
        let error = anyhow::Error::new(ConformanceError::FloorViolation(
            "check `x` quoted <masked>".to_string(),
        ));
        let reason = refusal_reason(&error);
        assert_eq!(
            reason,
            "the pack does not conform to the registered contract floor"
        );
        assert!(!reason.contains("<masked>"), "{reason}");
    }

    #[test]
    fn a_selector_must_pin_one_core_version() {
        for invalid in ["cli-assist", "cli-assist@", "@1.0.0", "a@1.0.0@2"] {
            assert!(split_selector(invalid).is_none(), "{invalid}");
        }
        assert_eq!(
            split_selector(" cli-assist@1.0.0 "),
            Some(("cli-assist", "1.0.0"))
        );
    }

    #[test]
    fn a_startup_pack_that_left_the_roots_is_refused_with_its_reason() {
        let root = tempfile::tempdir().unwrap();
        let startup = ResolvedPack {
            id: "gone".to_string(),
            version: "1.0.0".to_string(),
            hash: "sha256:gone".to_string(),
            source: SelectionSource::ExtensionRoot,
            directory: root.path().to_path_buf(),
            declared: None,
        };
        let context = ReplPackContext::with_startup(
            root.path(),
            Some(root.path().to_path_buf()),
            Some(startup),
        );
        let refused = context.startup_refusal().unwrap();
        assert!(refused.contains("--pack gone@1.0.0"), "{refused}");
        assert!(!refused.contains("PACK_CANARY"), "{refused}");
    }

    /// The extension root wins over the workspace for the same selector, the
    /// list shows one candidate, and a local pack is never admitted.
    #[test]
    fn one_selector_resolves_once_to_the_extension_root_and_a_local_is_never_admitted() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        let local = write_pack(
            &extension,
            "canary-pack",
            "1.0.0",
            &assist("canary-pack", "1.0.0", "r"),
        );
        let repository = write_pack(
            &workspace.join("packs"),
            "canary-pack",
            "1.0.0",
            &assist("canary-pack", "1.0.0", "PACK_CANARY_27"),
        );
        assert_ne!(local, repository, "the roots must hold different bytes");

        let locator = PackLocator::with_extension_root(&workspace, Some(extension.clone()));
        let located = locator.locate_pinned("canary-pack", "1.0.0", None).unwrap();
        assert_eq!(located.source, PackSource::Local);
        assert_eq!(located.hash, local);

        let selectable = locator.selectable("python-cli", "create");
        assert_eq!(
            selectable
                .iter()
                .filter(|pack| pack.id == "canary-pack")
                .count(),
            1,
            "{selectable:?}"
        );
        assert!(
            selectable
                .iter()
                .all(|pack| pack.source != PackSource::Admitted || pack.id != "canary-pack")
        );
    }

    /// An invalid higher candidate is refused with its reason; the lower one in
    /// the workspace is never adopted, and the reason does not quote the marker
    /// that sits outside the root.
    #[test]
    fn an_invalid_extension_pack_is_refused_without_falling_back_or_leaking_markers() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        // The direct layout holds the marker; the `packs/` layout holds a valid
        // pack. The selection reads the direct one, which must not leak.
        write_pack(
            &extension,
            "canary-pack",
            "1.0.0",
            &assist("canary-pack", "1.0.0", "PACK_CANARY_27"),
        );
        write_pack(
            &workspace.join("packs"),
            "canary-pack",
            "1.0.0",
            &assist("canary-pack", "1.0.0", "workspace-fallback"),
        );
        // Break the higher candidate's pin.
        std::fs::write(
            extension
                .join("canary-pack")
                .join("1.0.0")
                .join("pack.sha256"),
            "sha256:stale\n",
        )
        .unwrap();

        let locator = PackLocator::with_extension_root(&workspace, Some(extension.clone()));
        let error = locator
            .locate_pinned("canary-pack", "1.0.0", None)
            .unwrap_err();
        let reason = format!("{error:#}");
        assert!(reason.contains("pin does not match its bytes"), "{reason}");
        assert!(!reason.contains("PACK_CANARY_27"), "{reason}");
        assert!(!reason.contains("workspace-fallback"), "{reason}");
        assert!(
            locator
                .selectable("python-cli", "create")
                .iter()
                .all(|pack| pack.id != "canary-pack"),
            "the invalid candidate fell back to another pack"
        );
    }

    /// A retired candidate is refused, and the workspace copy is not adopted.
    #[test]
    fn a_retired_extension_pack_does_not_fall_back() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        write_pack(
            &extension,
            "canary-pack",
            "1.0.0",
            &assist("canary-pack", "1.0.0", "retired"),
        );
        std::fs::write(
            extension
                .join("canary-pack")
                .join("1.0.0")
                .join(crate::planner::pack::catalog::RETIRED_MARKER_FILE),
            b"",
        )
        .unwrap();
        write_pack(
            &workspace.join("packs"),
            "canary-pack",
            "1.0.0",
            &assist("canary-pack", "1.0.0", "workspace"),
        );

        let locator = PackLocator::with_extension_root(&workspace, Some(extension));
        let error = locator
            .locate_pinned("canary-pack", "1.0.0", None)
            .unwrap_err();
        assert!(
            refusal_reason(&error).contains("retired"),
            "{}",
            refusal_reason(&error)
        );
    }

    /// A pack for another profile or intent is refused with its reason and never
    /// answers for the requested route.
    #[test]
    fn a_pack_for_another_profile_or_intent_is_refused_with_its_reason() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        // A data profile pack, so the python-cli route must refuse it.
        write_pack(
            &extension,
            "canary-pack",
            "1.0.0",
            &assist_from("data-assist", "canary-pack", "1.0.0", "profile-mismatch"),
        );
        let context = ReplPackContext::new(workspace, Some(extension));
        let error = context
            .select("python-cli", "create", "canary-pack@1.0.0")
            .unwrap_err();
        let reason = refusal_reason(&error);
        assert!(
            reason.contains("data") && reason.contains("python-cli"),
            "{reason}"
        );
        assert!(!reason.contains("profile-mismatch"), "{reason}");
        assert!(
            !context
                .locator()
                .selectable("python-cli", "create")
                .iter()
                .any(|pack| pack.id == "canary-pack"),
            "a pack for another profile was listed as selectable"
        );
    }

    /// A selector the roots cannot read is refused by name; malformed selectors
    /// keep their own error.
    #[test]
    fn an_unreadable_selector_is_refused_without_selecting_another() {
        let temp = tempfile::tempdir().unwrap();
        let context = ReplPackContext::new(temp.path(), None);
        assert!(context.resolved_hash("canary-pack@1.0.0").is_none());
        assert!(context.resolved_hash("canary-pack").is_none());
        let reason = context.selector_refusal("canary-pack@1.0.0");
        assert!(!reason.is_empty(), "{reason}");
    }
}
