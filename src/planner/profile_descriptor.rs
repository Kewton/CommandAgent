use crate::planner::pack::PackProfile;
use crate::planner::profile::{DataProfile, DomainProfile, GenericProfile, ProfileId};
use crate::planner::profile_behavior::ProfileRuntime;
use crate::planner::profile_manifest::ManifestStatus;
use crate::planner::profiles::{community_mini_app, data, ingest, nextjs, python_cli};

pub const NEXTJS_PROFILE_ID: &str = nextjs::PROFILE_ID;
pub const COMMUNITY_MINI_APP_PROFILE_ID: &str = community_mini_app::PROFILE_ID;
pub const PYTHON_CLI_PROFILE_ID: &str = "python-cli";
pub const DATA_PROFILE_ID: &str = "data";
pub const INGEST_PROFILE_ID: &str = "ingest";
pub const GENERIC_PROFILE_ID: &str = "generic";

pub struct ProfileDescriptor {
    pub id: ProfileId,
    pub canonical: &'static str,
    pub aliases: &'static [&'static str],
    pub display_name_ja: &'static str,
    pub description_ja: &'static str,
    pub admission: fn() -> ManifestStatus,
    pub runtime: &'static dyn ProfileRuntime,
    pub domain: &'static dyn DomainProfile,
    pub contract_ref: Option<&'static str>,
    pub band_key: Option<&'static str>,
    pub pack_profile: Option<PackProfile>,
}

static NEXTJS_PROFILE: nextjs::NextjsProfile = nextjs::NextjsProfile;
static COMMUNITY_MINI_APP_PROFILE: community_mini_app::CommunityMiniAppProfile =
    community_mini_app::CommunityMiniAppProfile;
static PYTHON_CLI_PROFILE: python_cli::PythonCliProfile = python_cli::PythonCliProfile;
static DATA_PROFILE: DataProfile = DataProfile;
static INGEST_PROFILE: ingest::IngestProfile = ingest::IngestProfile;
static GENERIC_PROFILE: GenericProfile = GenericProfile;

pub static PROFILE_DESCRIPTORS: &[ProfileDescriptor] = &[
    ProfileDescriptor {
        id: ProfileId::Nextjs,
        canonical: NEXTJS_PROFILE_ID,
        aliases: &["next-js", "next.js"],
        display_name_ja: "Next.js",
        description_ja: "ブラウザー向け契約チェックを備えた Next.js App Router プロジェクト。",
        admission: nextjs_admission,
        runtime: &NEXTJS_PROFILE,
        domain: &NEXTJS_PROFILE,
        contract_ref: Some("docs/nextjs-profile-contract.md"),
        band_key: Some(NEXTJS_PROFILE_ID),
        pack_profile: Some(PackProfile::Nextjs),
    },
    ProfileDescriptor {
        id: ProfileId::CommunityMiniApp,
        canonical: COMMUNITY_MINI_APP_PROFILE_ID,
        aliases: &[],
        display_name_ja: COMMUNITY_MINI_APP_PROFILE_ID,
        description_ja: "許可済みの CommandAgent ランタイムプロファイル。",
        admission: admitted,
        runtime: &COMMUNITY_MINI_APP_PROFILE,
        domain: &COMMUNITY_MINI_APP_PROFILE,
        contract_ref: Some("docs/community-mini-app-profile-contract.md"),
        band_key: None,
        pack_profile: None,
    },
    ProfileDescriptor {
        id: ProfileId::PythonCli,
        canonical: PYTHON_CLI_PROFILE_ID,
        aliases: &["python", "py-cli", "py", "cli"],
        display_name_ja: "Python CLI",
        description_ja: "使用方法と動作を検証する Python コマンドラインツール。",
        admission: python_cli_admission,
        runtime: &PYTHON_CLI_PROFILE,
        domain: &PYTHON_CLI_PROFILE,
        contract_ref: Some("docs/cli-profile-contract.md"),
        band_key: Some(PYTHON_CLI_PROFILE_ID),
        pack_profile: Some(PackProfile::PythonCli),
    },
    ProfileDescriptor {
        id: ProfileId::Data,
        canonical: DATA_PROFILE_ID,
        aliases: &[],
        display_name_ja: "表形式データパイプライン",
        description_ja: "CSV または TSV の検査、変換、照合、レポート作成。",
        admission: data_admission,
        runtime: &DATA_PROFILE,
        domain: &DATA_PROFILE,
        contract_ref: Some("docs/dev/data-profile-contract.md"),
        band_key: Some(DATA_PROFILE_ID),
        pack_profile: Some(PackProfile::Data),
    },
    ProfileDescriptor {
        id: ProfileId::Ingest,
        canonical: INGEST_PROFILE_ID,
        aliases: &[],
        display_name_ja: "スナップショット取り込みパイプライン",
        description_ja: "ソースと候補件数を検証するオフライン・スナップショット抽出。",
        admission: ingest_admission,
        runtime: &INGEST_PROFILE,
        domain: &INGEST_PROFILE,
        contract_ref: Some("docs/ingest-profile-contract.md"),
        band_key: Some(INGEST_PROFILE_ID),
        pack_profile: Some(PackProfile::Ingest),
    },
    ProfileDescriptor {
        id: ProfileId::Generic,
        canonical: GENERIC_PROFILE_ID,
        aliases: &[],
        display_name_ja: "汎用",
        description_ja: "許可済みの専用プロファイル契約を使用しない一般的な作業。",
        admission: admitted,
        runtime: &GENERIC_PROFILE,
        domain: &GENERIC_PROFILE,
        contract_ref: None,
        band_key: None,
        pack_profile: None,
    },
];

pub fn descriptor(id: &ProfileId) -> Option<&'static ProfileDescriptor> {
    PROFILE_DESCRIPTORS
        .iter()
        .find(|candidate| &candidate.id == id)
        .or_else(|| crate::planner::extension_profiles::descriptor(id))
}

pub fn descriptor_for_name(name: &str) -> Option<&'static ProfileDescriptor> {
    let normalized = name.trim().to_ascii_lowercase();
    PROFILE_DESCRIPTORS
        .iter()
        .find(|candidate| {
            candidate.canonical == normalized || candidate.aliases.contains(&normalized.as_str())
        })
        .or_else(|| crate::planner::extension_profiles::descriptor_for_name(&normalized))
}

pub fn descriptor_for_domain(name: &str) -> &'static ProfileDescriptor {
    if let Some(extension) = crate::planner::extension_profiles::descriptor_for_name(name) {
        return extension;
    }
    PROFILE_DESCRIPTORS
        .iter()
        .filter(|candidate| candidate.id != ProfileId::Generic)
        .find(|candidate| candidate.domain.matches(name))
        .or_else(|| descriptor(&ProfileId::Generic))
        .expect("PROFILE_DESCRIPTORS must register the generic fallback")
}

/// Resolve the effective pack-profile identity for a selected runtime profile.
///
/// Compiled descriptors retain their explicit closed `PackProfile`. An
/// external descriptor gains only its already-registered exact draft ID; this
/// does not add it to the compiled registry or change its admission status.
pub fn pack_profile_for_name(name: &str) -> Option<PackProfile> {
    let descriptor = descriptor_for_name(name)?;
    descriptor.pack_profile.or_else(|| {
        crate::planner::extension_profiles::find(descriptor.canonical)
            .map(|profile| PackProfile::Draft(profile.id))
    })
}

/// Decode a canonical pack identity. Unlike runtime selection, aliases are not
/// accepted in pack bytes.
pub fn pack_profile_for_identity(name: &str) -> Option<PackProfile> {
    let normalized = name.trim().to_ascii_lowercase();
    if name != normalized {
        return None;
    }
    let descriptor = descriptor_for_name(&normalized)?;
    if descriptor.canonical != normalized {
        return None;
    }
    descriptor.pack_profile.or_else(|| {
        crate::planner::extension_profiles::find(descriptor.canonical)
            .map(|profile| PackProfile::Draft(profile.id))
    })
}

fn admitted() -> ManifestStatus {
    ManifestStatus::Admitted
}

fn nextjs_admission() -> ManifestStatus {
    nextjs::manifest_status(NEXTJS_PROFILE_ID).unwrap_or(ManifestStatus::Draft)
}

fn python_cli_admission() -> ManifestStatus {
    python_cli::manifest::get().metadata.status
}

fn data_admission() -> ManifestStatus {
    data::manifest::get().metadata.status
}

fn ingest_admission() -> ManifestStatus {
    ingest::manifest::get().metadata.status
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::tui::boundary_shell::band_catalog::BAND_VALUES;
    use crate::tui::boundary_shell::family_catalog::TASK_FAMILY_CATALOG;

    #[test]
    fn descriptors_have_unique_names_and_coherent_typed_implementations() {
        let mut names = BTreeSet::new();
        for profile in PROFILE_DESCRIPTORS {
            assert!(names.insert(profile.canonical), "{}", profile.canonical);
            for alias in profile.aliases {
                assert!(names.insert(alias), "duplicate profile alias {alias}");
                assert_eq!(
                    descriptor_for_name(alias).map(|item| &item.id),
                    Some(&profile.id)
                );
            }
            assert_eq!(ProfileId::parse(profile.canonical), profile.id);
            assert_eq!(profile.domain.id(), profile.canonical);
            assert_eq!(profile.runtime.profile_id(), profile.id);
            assert_eq!(
                descriptor(&profile.id).map(|item| item.canonical),
                Some(profile.canonical)
            );
        }
        assert_eq!(descriptor_for_domain("data-analysis").id, ProfileId::Data);
        assert_eq!(descriptor_for_domain("cli").id, ProfileId::Generic);
        assert_eq!(
            crate::planner::profile_admission::status("data-analysis"),
            ManifestStatus::Draft
        );
        assert_eq!(
            crate::planner::profile_admission::status("data-pipeline"),
            ManifestStatus::Draft
        );
    }

    #[test]
    fn formally_routed_admitted_descriptors_have_complete_catalog_links() {
        for profile in PROFILE_DESCRIPTORS
            .iter()
            .filter(|profile| profile.band_key.is_some())
        {
            assert_eq!((profile.admission)(), ManifestStatus::Admitted);
            assert!(profile.contract_ref.is_some(), "{}", profile.canonical);
            let band_key = profile.band_key.unwrap();
            assert!(
                BAND_VALUES.iter().any(|band| band.profile == band_key),
                "{} has no formal band",
                profile.canonical
            );
            assert!(
                TASK_FAMILY_CATALOG
                    .iter()
                    .any(|family| family.profile == profile.canonical),
                "{} has no task family",
                profile.canonical
            );
        }
    }

    #[test]
    fn linked_catalogs_and_pack_profiles_resolve_back_to_descriptors() {
        for band in BAND_VALUES {
            assert!(
                descriptor_for_name(band.profile).is_some(),
                "{}",
                band.profile
            );
        }
        for family in TASK_FAMILY_CATALOG {
            assert!(
                descriptor_for_name(family.profile).is_some(),
                "{}",
                family.profile
            );
        }
        for profile in PROFILE_DESCRIPTORS {
            if let Some(pack_profile) = profile.pack_profile {
                assert_eq!(pack_profile.as_str(), profile.canonical);
                assert_eq!(PackProfile::parse(profile.canonical), Some(pack_profile));
            }
        }
    }

    #[test]
    fn pack_identity_resolution_keeps_compiled_aliases_out_of_pack_bytes() {
        assert_eq!(
            pack_profile_for_name("python"),
            Some(PackProfile::PythonCli)
        );
        assert_eq!(
            pack_profile_for_identity(PYTHON_CLI_PROFILE_ID),
            Some(PackProfile::PythonCli)
        );
        assert_eq!(pack_profile_for_identity("python"), None);
        assert_eq!(pack_profile_for_identity(" python-cli "), None);
        assert_eq!(pack_profile_for_identity(GENERIC_PROFILE_ID), None);
        assert_eq!(pack_profile_for_identity("unregistered-draft"), None);
    }
}
