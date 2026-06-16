#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileId {
    Generic,
    NextJs,
    Python,
    Rust,
    Investigation,
    Docs,
    DataAnalysis,
    DataPipeline,
}

impl ProfileId {
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "generic" => Ok(Self::Generic),
            "nextjs" | "next.js" => Ok(Self::NextJs),
            "python" => Ok(Self::Python),
            "rust" => Ok(Self::Rust),
            "investigation" => Ok(Self::Investigation),
            "docs" | "documentation" => Ok(Self::Docs),
            "data-analysis" | "data_analysis" => Ok(Self::DataAnalysis),
            "data-pipeline" | "data_pipeline" => Ok(Self::DataPipeline),
            other => Err(ProfileError::UnknownProfile(other.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::NextJs => "nextjs",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Investigation => "investigation",
            Self::Docs => "docs",
            Self::DataAnalysis => "data-analysis",
            Self::DataPipeline => "data-pipeline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileContract {
    pub id: ProfileId,
    pub text: String,
    pub verifier_commands: Vec<String>,
    pub protected_path_prefixes: Vec<String>,
}

pub fn profile_contract(id: ProfileId) -> ProfileContract {
    match id {
        ProfileId::Generic => ProfileContract {
            id,
            text: "Keep changes scoped. Prefer Read/Bash inspection before editing. Use Write/Edit for file changes. End with deterministic checks when practical.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::NextJs => ProfileContract {
            id,
            text: "For Next.js work, preserve honest build scripts. New apps need package.json with next/react/react-dom dependencies, app/page.tsx or pages/index.tsx, and a build script that remains `next build`. If node_modules/.bin/next is missing, install dependencies when allowed or report dependency_missing; never fake build success.".to_string(),
            verifier_commands: vec!["npm run build".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Python => ProfileContract {
            id,
            text: "For Python work, keep modules importable, prefer small functions, and verify with pytest or direct local script execution when tests are not present.".to_string(),
            verifier_commands: vec!["python -m pytest".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Rust => ProfileContract {
            id,
            text: "For Rust work, keep Cargo.toml honest, use idiomatic modules, and verify with cargo test, cargo build, or cargo run when requested. For new minimal projects, create Cargo.toml and src/main.rs with Write/Edit instead of cargo init or cargo new.".to_string(),
            verifier_commands: vec!["cargo test".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Investigation => ProfileContract {
            id,
            text: "For investigation work, inspect first, preserve evidence, and avoid code changes unless the requested task explicitly asks for a fix.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Docs => ProfileContract {
            id,
            text: "For documentation work, keep claims tied to repository facts, update indexes when present, and avoid changing behavior code.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::DataAnalysis => ProfileContract {
            id,
            text: "For data analysis work, keep raw inputs immutable, write derived artifacts separately, and record assumptions and reproducible commands.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: data_protected_prefixes(),
        },
        ProfileId::DataPipeline => ProfileContract {
            id,
            text: "For data pipeline work, keep raw inputs immutable, separate extraction, transformation, and output steps, and make reruns deterministic.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: data_protected_prefixes(),
        },
    }
}

pub fn profile_contract_text(profile: &str) -> Result<String, ProfileError> {
    Ok(profile_contract(ProfileId::parse(profile)?).text)
}

pub fn profile_verifier_commands(profile: &str) -> Result<Vec<String>, ProfileError> {
    Ok(profile_contract(ProfileId::parse(profile)?).verifier_commands)
}

pub fn protected_by_profile(profile: &str, path: &str) -> Result<bool, ProfileError> {
    let contract = profile_contract(ProfileId::parse(profile)?);
    Ok(contract
        .protected_path_prefixes
        .iter()
        .any(|prefix| path == prefix || path.starts_with(&format!("{prefix}/"))))
}

fn data_protected_prefixes() -> Vec<String> {
    vec![
        "raw".to_string(),
        "data/raw".to_string(),
        "input".to_string(),
        "inputs".to_string(),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    UnknownProfile(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProfile(profile) => write!(f, "unknown profile: {profile}"),
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_mvp_profiles() {
        for profile in [
            "generic",
            "nextjs",
            "python",
            "rust",
            "investigation",
            "docs",
            "data-analysis",
            "data-pipeline",
        ] {
            assert!(ProfileId::parse(profile).is_ok(), "{profile}");
        }
    }

    #[test]
    fn nextjs_profile_preserves_honest_build_contract() {
        let contract = profile_contract(ProfileId::NextJs);

        assert!(contract.text.contains("next/react/react-dom"));
        assert!(contract.text.contains("never fake build success"));
        assert_eq!(contract.verifier_commands, vec!["npm run build"]);
    }

    #[test]
    fn rust_profile_keeps_scaffolding_in_file_tools() {
        let contract = profile_contract(ProfileId::Rust);

        assert!(contract.text.contains("Cargo.toml"));
        assert!(contract.text.contains("src/main.rs"));
        assert!(contract.text.contains("instead of cargo init or cargo new"));
    }

    #[test]
    fn data_profiles_protect_raw_inputs() {
        assert!(protected_by_profile("data-analysis", "data/raw/source.csv").unwrap());
        assert!(protected_by_profile("data-pipeline", "raw/source.csv").unwrap());
        assert!(!protected_by_profile("data-analysis", "derived/report.csv").unwrap());
    }

    #[test]
    fn unknown_profile_is_error() {
        let err = ProfileId::parse("legacy").unwrap_err();

        assert_eq!(err, ProfileError::UnknownProfile("legacy".to_string()));
    }
}
