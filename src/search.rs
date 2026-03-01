use regex::Regex;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use crate::exclude::is_excluded;
use crate::filedetect::is_likely_text_file;
use crate::model::SearchResult;

/// Extract a literal prefix from a regex string for fast pre-filtering.
/// Returns `Some(prefix)` if the prefix is >= 3 characters, else `None`.
pub fn extract_literal_prefix(regex_str: &str) -> Option<String> {
    if regex_str.contains("(?i)") {
        return None;
    }

    let mut s = regex_str;
    if s.starts_with('^') {
        s = &s[1..];
    }
    if s.starts_with("(?m)") {
        s = &s[4..];
    }
    if s.ends_with('$') {
        s = &s[..s.len() - 1];
    }

    let mut literal = String::new();
    let mut escaped = false;

    for c in s.chars() {
        if escaped {
            literal.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '.' | '*' | '+' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' => {
                break;
            }
            _ => literal.push(c),
        }
    }

    if literal.len() >= 3 {
        Some(literal)
    } else {
        None
    }
}

/// Search the content of a file line-by-line using the given regex.
fn search_lines(path: &str, content: &[u8], pattern: &Regex) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut line_num = 0usize;
    let mut start = 0;

    while start <= content.len() {
        line_num += 1;
        let end = content[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|pos| start + pos + 1)
            .unwrap_or(content.len());

        // Exclude trailing \n (and \r if present) for the line text
        let line_end = if end > start && content[end - 1] == b'\n' {
            if end >= 2 && content[end - 2] == b'\r' {
                end - 2
            } else {
                end - 1
            }
        } else {
            end
        };

        let line_bytes = &content[start..line_end];
        let line_str = String::from_utf8_lossy(line_bytes);

        if pattern.is_match(&line_str) {
            let match_text = pattern
                .find(&line_str)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            results.push(SearchResult {
                file_path: path.to_string(),
                line_num,
                line_text: line_str.into_owned(),
                match_text,
            });
        }

        if end == content.len() && (start == end || content[end - 1] != b'\n') {
            break;
        }
        start = end;
    }

    results
}

/// Read entire small file into memory, quick-reject, then search lines.
fn search_small_file(path: &str, pattern: &Regex) -> Vec<SearchResult> {
    let content = match fs::read(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let content_str = String::from_utf8_lossy(&content);
    if !pattern.is_match(&content_str) {
        return Vec::new();
    }

    search_lines(path, &content, pattern)
}

/// Stream a large file line-by-line to avoid loading it all into memory.
fn search_large_file(path: &str, pattern: &Regex) -> Vec<SearchResult> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::with_capacity(128 * 1024, file);
    let mut results = Vec::new();
    let mut line_num = 0usize;
    let mut buf = Vec::with_capacity(1024);

    let mut reader = reader;
    loop {
        buf.clear();
        let bytes_read = match reader.read_until(b'\n', &mut buf) {
            Ok(n) => n,
            Err(_) => break,
        };
        if bytes_read == 0 {
            break;
        }
        line_num += 1;

        // Strip trailing newline chars for matching
        let line_end = if buf.ends_with(b"\r\n") {
            buf.len() - 2
        } else if buf.ends_with(b"\n") {
            buf.len() - 1
        } else {
            buf.len()
        };

        let line_str = String::from_utf8_lossy(&buf[..line_end]);
        if pattern.is_match(&line_str) {
            let match_text = pattern
                .find(&line_str)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            results.push(SearchResult {
                file_path: path.to_string(),
                line_num,
                line_text: line_str.into_owned(),
                match_text,
            });
        }
    }

    results
}

/// Use a literal prefix for fast byte-level pre-filtering before regex.
fn search_with_prefilter(path: &str, pattern: &Regex, literal: &str) -> Vec<SearchResult> {
    let content = match fs::read(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let literal_bytes = literal.as_bytes();
    if !content
        .windows(literal_bytes.len())
        .any(|w| w == literal_bytes)
    {
        return Vec::new();
    }

    let content_str = String::from_utf8_lossy(&content);
    if !pattern.is_match(&content_str) {
        return Vec::new();
    }

    search_lines(path, &content, pattern)
}

/// Recursively collect files from the directory tree.
fn collect_files(
    root: &Path,
    exclude_list: &[String],
) -> Vec<(PathBuf, u64)> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            if is_excluded(&path_str, exclude_list) {
                continue;
            }

            if path.is_dir() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str == ".git" || name_str == ".cache" || name_str == "node_modules" {
                    continue;
                }
                stack.push(path);
            } else if path.is_file() {
                if !is_likely_text_file(&path) {
                    continue;
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((path, size));
            }
        }
    }

    files
}

const LARGE_FILE_THRESHOLD: u64 = 10 * 1024 * 1024; // 10 MB

/// Perform an adaptive parallel search across the directory tree.
pub fn perform_search_adaptive(
    root_path: &str,
    pattern: &Regex,
    exclude_list: &[String],
) -> Result<Vec<SearchResult>, String> {
    let root = Path::new(root_path);
    if !root.exists() {
        return Err(format!("path does not exist: {}", root_path));
    }

    let literal = extract_literal_prefix(pattern.as_str());
    let has_literal = literal.is_some();
    let files = collect_files(root, exclude_list);

    let num_workers = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let pattern = Arc::new(pattern.clone());
    let literal = Arc::new(literal);

    let (job_tx, job_rx) = mpsc::channel::<(PathBuf, u64)>();
    let (result_tx, result_rx) = mpsc::channel::<Vec<SearchResult>>();

    // Wrap the receiver in Arc<Mutex<>> so multiple workers can share it
    let job_rx = Arc::new(std::sync::Mutex::new(job_rx));

    let mut handles = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        let job_rx = Arc::clone(&job_rx);
        let pattern = Arc::clone(&pattern);
        let literal = Arc::clone(&literal);
        let result_tx = result_tx.clone();

        handles.push(thread::spawn(move || {
            loop {
                let job = {
                    let rx = job_rx.lock().unwrap();
                    rx.recv()
                };
                match job {
                    Ok((path, size)) => {
                        let path_str = path.to_string_lossy().to_string();
                        let file_results = if size > LARGE_FILE_THRESHOLD {
                            search_large_file(&path_str, &pattern)
                        } else if has_literal {
                            search_with_prefilter(
                                &path_str,
                                &pattern,
                                literal.as_deref().unwrap(),
                            )
                        } else {
                            search_small_file(&path_str, &pattern)
                        };
                        if !file_results.is_empty() {
                            let _ = result_tx.send(file_results);
                        }
                    }
                    Err(_) => break, // Channel closed
                }
            }
        }));
    }

    // Drop our copy of result_tx so result_rx closes when workers finish
    drop(result_tx);

    // Send all jobs
    for (path, size) in files {
        let _ = job_tx.send((path, size));
    }
    drop(job_tx);

    // Collect results
    let mut all_results = Vec::new();
    for batch in result_rx {
        all_results.extend(batch);
    }

    for h in handles {
        let _ = h.join();
    }

    // Sort results by file path then line number for deterministic output
    all_results.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.line_num.cmp(&b.line_num))
    });

    Ok(all_results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_literal_prefix_plain() {
        assert_eq!(
            extract_literal_prefix("hello"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn test_extract_literal_prefix_case_insensitive() {
        assert_eq!(extract_literal_prefix("(?i)hello"), None);
    }

    #[test]
    fn test_extract_literal_prefix_metachar() {
        // "he" is only 2 chars, less than 3 → None
        assert_eq!(extract_literal_prefix("he.*lo"), None);
    }

    #[test]
    fn test_extract_literal_prefix_escaped() {
        assert_eq!(
            extract_literal_prefix("hel\\.lo"),
            Some("hel.lo".to_string())
        );
    }

    #[test]
    fn test_extract_literal_prefix_short() {
        assert_eq!(
            extract_literal_prefix("abc"),
            Some("abc".to_string())
        );
    }

    #[test]
    fn test_extract_literal_prefix_too_short() {
        assert_eq!(extract_literal_prefix("ab"), None);
    }
}
