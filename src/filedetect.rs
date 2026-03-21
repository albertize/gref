use std::path::Path;

/// Known text file extensions (sorted for binary search).
const TEXT_EXTENSIONS: &[&str] = &[
    ".asm", ".bat", ".c", ".cfg", ".clj", ".cmake", ".conf", ".cpp", ".cs",
    ".css", ".csv", ".dart", ".diff", ".elm", ".env", ".erl", ".ex", ".exs",
    ".go", ".graphql", ".h", ".hcl", ".hpp", ".html", ".ini", ".java", ".js",
    ".json", ".jsx", ".kt", ".less", ".lock", ".log", ".lua", ".md", ".mk",
    ".nix", ".patch", ".php", ".pl", ".proto", ".ps1", ".py", ".rb", ".rs",
    ".rst", ".sass", ".sbt", ".scala", ".scss", ".sh", ".sql", ".svelte",
    ".swift", ".tf", ".toml", ".ts", ".tsv", ".tsx", ".txt", ".vue", ".xml",
    ".yaml", ".yml", ".zig",
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

/// Extension-only classification. Returns:
/// - `Some(true)` if known text extension
/// - `Some(false)` if known binary extension
/// - `None` if unknown (needs content-based detection)
pub fn classify_by_extension(path: &Path) -> Option<bool> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    let lower = ext.to_lowercase();
    let mut buf = String::with_capacity(lower.len() + 1);
    buf.push('.');
    buf.push_str(&lower);
    if is_known_text(&buf) {
        return Some(true);
    }
    if is_known_binary(&buf) {
        return Some(false);
    }
    None
}

/// Fast binary detection on an already-loaded buffer using SIMD-accelerated null-byte scan.
/// Returns true if the content appears to be binary (contains \0 in the first 512 bytes).
pub fn is_binary_content(content: &[u8]) -> bool {
    let check_len = content.len().min(512);
    memchr::memchr(0, &content[..check_len]).is_some()
}

/// Determine if a file is likely a text file based on extension or content.
pub fn is_likely_text_file(path: &Path) -> bool {
    if let Some(is_text) = classify_by_extension(path) {
        return is_text;
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
