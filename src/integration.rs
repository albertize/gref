use crate::model::SearchResult;
use std::fs;
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

fn push_path_bytes(out: &mut Vec<u8>, path: &Path) {
    #[cfg(unix)]
    out.extend_from_slice(path.as_os_str().as_bytes());

    #[cfg(not(unix))]
    out.extend_from_slice(path.to_string_lossy().as_bytes());
}

/// Write a selected result in a format Vimscript can parse without ambiguity:
/// first line is the 1-based line number, remaining bytes are the path.
pub fn write_vim_result(result_path: &Path, result: &SearchResult) -> Result<(), String> {
    let mut out = Vec::new();
    writeln!(&mut out, "{}", result.line_num)
        .map_err(|e| format!("failed to encode Vim result: {}", e))?;
    push_path_bytes(&mut out, result.path());
    fs::write(result_path, out).map_err(|e| {
        format!(
            "failed to write Vim result to {}: {}",
            result_path.display(),
            e
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn write_vim_result_preserves_colons_in_path() {
        let name = format!(
            "gref_test_vim_result_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        let result = SearchResult::from_display_path("dir/file:with:colon.rs", 42, "foo");

        write_vim_result(&path, &result).unwrap();

        let bytes = fs::read(&path).unwrap();
        let split = bytes.iter().position(|&b| b == b'\n').unwrap();
        assert_eq!(&bytes[..split], b"42");
        assert_eq!(&bytes[split + 1..], b"dir/file:with:colon.rs");

        let _ = fs::remove_file(path);
    }
}
