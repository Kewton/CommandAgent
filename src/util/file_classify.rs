use std::path::Path;

pub fn is_likely_text_path(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return true;
    };

    !matches!(
        ext.to_ascii_lowercase().as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "pdf"
            | "zip"
            | "gz"
            | "tar"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "mp3"
            | "mp4"
            | "mov"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_binary_extensions() {
        assert!(!is_likely_text_path(Path::new("image.png")));
        assert!(!is_likely_text_path(Path::new("font.woff2")));
    }

    #[test]
    fn defaults_unknown_or_extensionless_to_text() {
        assert!(is_likely_text_path(Path::new("README")));
        assert!(is_likely_text_path(Path::new("main.rs")));
    }
}
