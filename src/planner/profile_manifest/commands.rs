//! Direct CLI backends for validating and initializing external manifests.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::overlay;
use super::source::{
    EXTENSION_MANIFEST_FILE, EXTENSION_OVERLAY_FILE, EXTENSION_PROFILES_DIRECTORY,
    ExtensionManifestError, ManifestSource, decode, read_optional, reject_registered_identity,
};

pub fn validate_file(path: &Path) -> Result<(), ExtensionManifestError> {
    let bytes = read_optional(path)?.ok_or_else(|| ExtensionManifestError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "file does not exist"),
    })?;
    let directory = path
        .parent()
        .ok_or_else(|| ExtensionManifestError::Invalid {
            path: path.to_path_buf(),
            reason: "must have a profile directory parent".to_string(),
        })?;
    if path
        .file_name()
        .is_some_and(|name| name == EXTENSION_OVERLAY_FILE)
    {
        overlay::decode(directory, path, &bytes, ManifestSource::Local)?;
    } else {
        decode(directory, path, &bytes)?;
    }
    Ok(())
}

pub fn init_profile(extension_root: &Path, id: &str) -> Result<PathBuf, ExtensionManifestError> {
    reject_registered_identity(id).map_err(|reason| ExtensionManifestError::Invalid {
        path: extension_root.to_path_buf(),
        reason,
    })?;
    require_existing_directory(extension_root, "must be an existing, non-symlink directory")?;

    let profiles = extension_root.join(EXTENSION_PROFILES_DIRECTORY);
    create_or_require_directory(&profiles)?;
    let profile = profiles.join(id);
    create_or_require_directory(&profile)?;
    let path = profile.join(EXTENSION_MANIFEST_FILE);
    let template = template(id);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(ExtensionManifestError::Invalid {
                path,
                reason: "already exists; refusing to overwrite it".to_string(),
            });
        }
        Err(source) => {
            return Err(ExtensionManifestError::Io { path, source });
        }
    };
    file.write_all(template.as_bytes())
        .map_err(|source| ExtensionManifestError::Io {
            path: path.clone(),
            source,
        })?;
    file.flush().map_err(|source| ExtensionManifestError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

pub fn template(id: &str) -> String {
    format!(
        "[metadata]\n\
id = \"{id}\"\n\
display_name = \"{id}\"\n\
schema_version = \"v2\"\n\
task_family = \"unknown\"\n\
[plan]\n\
intent = \"create\"\n\
phases = [{{ id = \"implementation\", prompt = \"Complete the requested work for {{goal}}.\" }}]\n\
[artifacts]\n\
required = [\"README.md\"]\n\
[guidance.variants.default]\n\
triggers = [{{ condition = \"always\" }}]\n\
messages = {{ instruction = \"Keep the implementation scoped to the requested goal.\" }}\n\
[[checks.final]]\n\
id = \"scaffold_files_present\"\n\
params = {{ files = [\"README.md\"] }}\n"
    )
}

fn create_or_require_directory(path: &Path) -> Result<(), ExtensionManifestError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            require_existing_directory(path, "must be a non-symlink directory")
        }
        Err(source) => Err(ExtensionManifestError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn require_existing_directory(path: &Path, reason: &str) -> Result<(), ExtensionManifestError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ExtensionManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        Err(ExtensionManifestError::Root {
            path: path.to_path_buf(),
            reason: reason.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_v2_template_is_bounded_and_valid() {
        let root = tempfile::tempdir().unwrap();
        let path = init_profile(root.path(), "neutral-profile").unwrap();
        let body = fs::read_to_string(&path).unwrap();

        assert!(
            body.lines().count() <= 20,
            "{} lines\n{body}",
            body.lines().count()
        );
        validate_file(&path).unwrap();
    }
}
