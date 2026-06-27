use crate::model::SearchResult;
use regex::Regex;
use std::fs::{self, OpenOptions};
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

fn write_status(result_path: &Path, status: &str, body: &[u8]) -> Result<(), String> {
    let mut out = Vec::new();
    writeln!(&mut out, "{}", status).map_err(|e| format!("failed to encode Vim result: {}", e))?;
    out.extend_from_slice(body);

    let parent = result_path.parent().unwrap_or_else(|| Path::new("."));
    let name = result_path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_else(|| "gref_vim_result".into());
    let mut last_error = None;

    for attempt in 0..100 {
        let tmp = parent.join(format!(".{}.{}.{}.tmp", name, std::process::id(), attempt));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(mut file) => {
                let write_result = file
                    .write_all(&out)
                    .and_then(|()| file.sync_all())
                    .map_err(|e| e.to_string());
                if let Err(e) = write_result {
                    let _ = fs::remove_file(&tmp);
                    return Err(format!(
                        "failed to write Vim result to {}: {}",
                        result_path.display(),
                        e
                    ));
                }
                if let Err(e) = fs::rename(&tmp, result_path) {
                    let _ = fs::remove_file(&tmp);
                    return Err(format!(
                        "failed to replace Vim result at {}: {}",
                        result_path.display(),
                        e
                    ));
                }
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last_error = Some(e);
            }
            Err(e) => {
                return Err(format!(
                    "failed to create Vim result temp file near {}: {}",
                    result_path.display(),
                    e
                ));
            }
        }
    }

    Err(format!(
        "failed to create unique Vim result temp file near {}: {}",
        result_path.display(),
        last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "too many attempts".to_string())
    ))
}

fn write_simple_status(result_path: &Path, status: &str) -> Result<(), String> {
    write_status(result_path, status, &[])
}

fn result_column(result: &SearchResult, pattern: &Regex) -> usize {
    pattern
        .find(&result.line_text)
        .map(|m| m.start() + 1)
        .unwrap_or(1)
}

/// Write a selected result in a format Vimscript can parse without ambiguity:
/// first line is "selected", then 1-based line number, 1-based byte column,
/// then remaining bytes are the path.
pub fn write_vim_selected_result(
    result_path: &Path,
    result: &SearchResult,
    pattern: &Regex,
) -> Result<(), String> {
    let mut body = Vec::new();
    writeln!(&mut body, "{}", result.line_num)
        .map_err(|e| format!("failed to encode Vim result: {}", e))?;
    writeln!(&mut body, "{}", result_column(result, pattern))
        .map_err(|e| format!("failed to encode Vim result: {}", e))?;
    push_path_bytes(&mut body, result.path());
    write_status(result_path, "selected", &body)
}

pub fn write_vim_no_results(result_path: &Path) -> Result<(), String> {
    write_simple_status(result_path, "none")
}

pub fn write_vim_replaced(result_path: &Path) -> Result<(), String> {
    write_simple_status(result_path, "replaced")
}

pub fn write_vim_cancelled(result_path: &Path) -> Result<(), String> {
    write_simple_status(result_path, "cancelled")
}

pub fn write_vim_error(result_path: &Path, message: &str) -> Result<(), String> {
    write_status(result_path, "error", message.as_bytes())
}

/// Deprecated compatibility writer for the original Vim result format.
#[allow(dead_code)]
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

    #[test]
    fn write_vim_selected_result_includes_line_column_and_path() {
        let name = format!(
            "gref_test_vim_selected_result_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        let result = SearchResult::from_display_path("dir/file.rs", 42, "abc foo");
        let pattern = Regex::new("foo").unwrap();

        write_vim_selected_result(&path, &result, &pattern).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes, b"selected\n42\n5\ndir/file.rs");

        let _ = fs::remove_file(path);
    }
}
