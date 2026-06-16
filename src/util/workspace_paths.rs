use std::path::{Path, PathBuf};

pub fn state_dir(cwd: &Path) -> PathBuf {
    cwd.join(".commandagent")
}

pub fn plans_dir(cwd: &Path) -> PathBuf {
    state_dir(cwd).join("plans")
}

pub fn repairs_dir(cwd: &Path) -> PathBuf {
    state_dir(cwd).join("repairs")
}

pub fn sessions_dir(cwd: &Path) -> PathBuf {
    state_dir(cwd).join("sessions")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_state_subdirs() {
        let cwd = Path::new("/workspace");

        assert_eq!(state_dir(cwd), PathBuf::from("/workspace/.commandagent"));
        assert_eq!(
            plans_dir(cwd),
            PathBuf::from("/workspace/.commandagent/plans")
        );
        assert_eq!(
            repairs_dir(cwd),
            PathBuf::from("/workspace/.commandagent/repairs")
        );
        assert_eq!(
            sessions_dir(cwd),
            PathBuf::from("/workspace/.commandagent/sessions")
        );
    }
}
