/// Check if a path matches any pattern in the exclusion list.
///
/// Handles:
/// - Directory patterns ending with `/` (e.g. `media/`)
/// - Extension patterns starting with `*.` (e.g. `*.log`)
/// - Exact filename matches (e.g. `file.txt`)
pub fn is_excluded(path: &str, exclude_list: &[String]) -> bool {
    let normalized = if path.contains('\\') {
        std::borrow::Cow::Owned(path.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(path)
    };

    for pattern in exclude_list {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }

        // Directory exclusion: pattern ends with "/"
        if let Some(pat_no_slash) = pattern.strip_suffix('/') {
            if normalized.contains(pattern) {
                return true;
            }
            // Check if normalized itself is the directory
            if normalized.ends_with(pat_no_slash) {
                return true;
            }
            continue;
        }

        // Extension exclusion: pattern starts with "*."
        if pattern.starts_with("*.") {
            if normalized.ends_with(&pattern[1..]) {
                return true;
            }
            continue;
        }

        // Exact filename match
        let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
        if file_name == pattern {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_excluded() {
        let exclude: Vec<String> = vec![
            ".git".into(),
            "*.log".into(),
            "media/".into(),
            "file.txt".into(),
        ];

        let cases = vec![
            ("/home/user/project/.git", true),
            ("/home/user/project/media/image.png", true),
            ("/home/user/project/file.txt", true),
            ("/home/user/project/notes.log", true),
            ("/home/user/project/notes.txt", false),
            ("/home/user/project/src/main.go", false),
        ];

        for (path, expected) in cases {
            assert_eq!(
                is_excluded(path, &exclude),
                expected,
                "is_excluded({:?}) should be {}",
                path,
                expected
            );
        }
    }
}
