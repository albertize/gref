use std::path::Path;

/// Known text file extensions (sorted for binary search).
const TEXT_EXTENSIONS: &[&str] = &[
    ".bat", ".c", ".cfg", ".conf", ".cpp", ".cs", ".css", ".go", ".h", ".hpp",
    ".html", ".ini", ".java", ".js", ".json", ".md", ".php", ".ps1", ".py",
    ".rb", ".rs", ".rst", ".sh", ".ts", ".txt", ".xml", ".yaml", ".yml",
];

/// Known binary file extensions (sorted for binary search).
const BINARY_EXTENSIONS: &[&str] = &[
    ".7z", ".a", ".aac", ".accdb", ".apk", ".avi", ".bin", ".bmp", ".bz2",
    ".cab", ".cache", ".class", ".cr2", ".crt", ".db", ".dbf", ".deb",
    ".der", ".dex", ".dll", ".dmg", ".doc", ".docx", ".dylib", ".ear",
    ".eot", ".exe", ".flac", ".flv", ".gif", ".ico", ".ipa", ".iso",
    ".jar", ".jks", ".jpg", ".jpeg", ".keystore", ".la", ".lib", ".lo",
    ".lzma", ".m4a", ".m4v", ".mdb", ".mkv", ".mov", ".mp3", ".mp4",
    ".nef", ".o", ".obj", ".ogg", ".otf", ".p12", ".pdf", ".pfx", ".png",
    ".psd", ".pyc", ".pyo", ".rar", ".raw", ".rpm", ".so", ".sqlite",
    ".sqlite3", ".svg", ".tar", ".temp", ".tif", ".tiff", ".tmp", ".ttf",
    ".war", ".wav", ".webm", ".webp", ".wma", ".wmv", ".woff", ".woff2",
    ".xls", ".xlsx", ".xz", ".zip",
];

fn is_known_text(ext: &str) -> bool {
    TEXT_EXTENSIONS.binary_search(&ext).is_ok()
}

fn is_known_binary(ext: &str) -> bool {
    BINARY_EXTENSIONS.binary_search(&ext).is_ok()
}

/// Determine if a file is likely a text file based on extension or content.
pub fn is_likely_text_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = format!(".{}", ext.to_lowercase());
        if is_known_text(&ext_lower) {
            return true;
        }
        if is_known_binary(&ext_lower) {
            return false;
        }
    }

    // Unknown extension — check content
    is_text_file_content(path)
}

/// Check if the first 512 bytes of a file look like text.
fn is_text_file_content(path: &Path) -> bool {
    use std::fs::File;
    use std::io::Read;

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut buf = [0u8; 512];
    let n = match file.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return false,
    };

    if n == 0 {
        return false;
    }

    for &b in &buf[..n] {
        if b == 0 || (b < 32 && b != 9 && b != 10 && b != 13) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_known_text_ext() {
        assert!(is_likely_text_file(Path::new("foo.rs")));
        assert!(is_likely_text_file(Path::new("bar.json")));
        assert!(is_likely_text_file(Path::new("script.sh")));
    }

    #[test]
    fn test_known_binary_ext() {
        assert!(!is_likely_text_file(Path::new("image.png")));
        assert!(!is_likely_text_file(Path::new("archive.zip")));
        assert!(!is_likely_text_file(Path::new("font.woff2")));
    }

    #[test]
    fn test_content_detection() {
        let dir = std::env::temp_dir();

        // Text content
        let text_path = dir.join("gref_test_text.noext");
        fs::write(&text_path, b"Hello, world!\nSecond line.\n").unwrap();
        assert!(is_likely_text_file(&text_path));
        let _ = fs::remove_file(&text_path);

        // Binary content (contains null byte)
        let bin_path = dir.join("gref_test_bin.noext");
        fs::write(&bin_path, b"Hello\x00world").unwrap();
        assert!(!is_likely_text_file(&bin_path));
        let _ = fs::remove_file(&bin_path);
    }
}
