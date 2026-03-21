use memchr::memmem;
use regex::bytes::Regex as BytesRegex;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::exclude::is_excluded;
use crate::filedetect;
use crate::gitignore::{self, GitIgnore};
use crate::model::SearchResult;

/// Extract the longest literal substring from a regex for fast pre-filtering.
/// Returns `Some(literal)` if the longest run is >= 3 characters, else `None`.
pub fn extract_longest_literal(regex_str: &str) -> Option<String> {
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

    let mut best = String::new();
    let mut current = String::new();
    let mut escaped = false;

    for c in s.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '.' | '*' | '+' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' => {
                if current.len() > best.len() {
                    best.clone_from(&current);
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if current.len() > best.len() {
        best = current;
    }

    if best.len() >= 3 {
        Some(best)
    } else {
        None
    }
}

/// Maximum file size to search (256 MB). Larger files are skipped.
const MAX_FILE_SIZE: u64 = 256 * 1024 * 1024;

/// Search file content using whole-buffer regex matching.
///
/// Instead of iterating line-by-line and restarting the regex engine per line,
/// feeds the entire buffer to `find_iter()` at once. This lets the regex engine's
/// internal SIMD and literal optimizations (Teddy, Aho-Corasick, Boyer-Moore)
/// work on the full buffer, skipping non-matching regions at hardware speed.
///
/// Line boundaries are found only for matching lines using SIMD-accelerated
/// `memchr`/`memrchr`. Line numbers are counted incrementally.
fn search_file(
    path: &str,
    content: &[u8],
    pattern: &BytesRegex,
    finder: Option<&memmem::Finder>,
) -> Vec<SearchResult> {
    if content.is_empty() {
        return Vec::new();
    }

    // Whole-file literal quick-reject via SIMD-accelerated memmem
    if let Some(f) = finder {
        if f.find(content).is_none() {
            return Vec::new();
        }
    }

    // Whole-buffer regex search — the core rg-style optimization
    let mut results = Vec::new();
    let mut last_line_start = usize::MAX;
    let mut last_counted_pos = 0;
    let mut running_line_num: usize = 1;

    for m in pattern.find_iter(content) {
        // Find line start: SIMD backward scan for newline
        let line_start = memchr::memrchr(b'\n', &content[..m.start()])
            .map(|p| p + 1)
            .unwrap_or(0);

        // Dedup: skip if same line as previous match
        if line_start == last_line_start {
            continue;
        }
        last_line_start = line_start;

        // Find line end: SIMD forward scan for newline
        let raw_end = memchr::memchr(b'\n', &content[m.end()..])
            .map(|p| m.end() + p)
            .unwrap_or(content.len());

        // Strip trailing \r (Windows line endings)
        let line_end = if raw_end > line_start && content[raw_end - 1] == b'\r' {
            raw_end - 1
        } else {
            raw_end
        };

        // Incremental line counting via SIMD-accelerated newline scan
        running_line_num +=
            memchr::memchr_iter(b'\n', &content[last_counted_pos..line_start]).count();
        last_counted_pos = line_start;

        let line_text = String::from_utf8_lossy(&content[line_start..line_end]).into_owned();
        results.push(SearchResult {
            file_path: path.to_string(),
            line_num: running_line_num,
            line_text,
        });
    }

    results
}

/// Directories to always skip during walk (sorted for binary search).
const SKIP_DIRS: &[&str] = &[
    ".cache", ".git", ".hg", ".mypy_cache", ".next", ".nuxt",
    ".pytest_cache", ".svn", ".tox", ".venv",
    "__pycache__", "node_modules", "target", "venv",
];

/// Check if a directory entry has the Windows hidden file attribute.
#[cfg(windows)]
fn is_hidden_windows(entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    entry
        .metadata()
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
        .unwrap_or(false)
}

/// Recursively walk the directory tree, dispatching files to the job channel.
///
/// Skips hidden files/dirs when `skip_hidden` is true. On Unix, hidden means
/// name starts with `.`. On Windows, also checks the `FILE_ATTRIBUTE_HIDDEN`
/// file attribute (e.g. `AppData`, `$Recycle.Bin`).
///
/// Loads and applies `.gitignore`, `.ignore`, and `.grefignore` rules hierarchically
/// when `use_gitignore` is true.
///
/// Zero-copy path filtering: OsStr-based checks (hidden, SKIP_DIRS, gitignore)
/// run on `entry.file_name()` before `entry.path()` allocates the full PathBuf.
/// For files with unknown extensions, content-based binary detection is deferred
/// to the worker threads (which already have the file in memory).
fn walk_and_dispatch(
    root: &Path,
    exclude_list: &[String],
    skip_hidden: bool,
    use_gitignore: bool,
    job_tx: &mpsc::Sender<(PathBuf, u64)>,
) {
    let initial_ignore = if use_gitignore {
        let mut ig = gitignore::load_ancestor_gitignores(root);
        ig = ig.merge_dir(root);
        Arc::new(ig)
    } else {
        Arc::new(GitIgnore::empty())
    };

    let mut stack: Vec<(PathBuf, Arc<GitIgnore>)> = vec![(root.to_path_buf(), initial_ignore)];

    while let Some((dir, inherited_ignore)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            // --- Phase 1: OsStr-only checks (no path allocation) ---
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if skip_hidden {
                if name_str.starts_with('.') {
                    continue;
                }
                #[cfg(windows)]
                if is_hidden_windows(&entry) {
                    continue;
                }
            }

            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if ft.is_dir() {
                if SKIP_DIRS.binary_search(&name_str.as_ref()).is_ok() {
                    continue;
                }
                if inherited_ignore.is_ignored(&name_str, true) {
                    continue;
                }
                let path = entry.path();
                // Load ignore files from this child directory
                let child_ignore = if use_gitignore {
                    Arc::new(inherited_ignore.merge_dir(&path))
                } else {
                    inherited_ignore.clone()
                };
                stack.push((path, child_ignore));
            } else if ft.is_file() {
                if inherited_ignore.is_ignored(&name_str, false) {
                    continue;
                }

                // --- Phase 2: Extension-only classification (no file I/O) ---
                let path = entry.path();
                match filedetect::classify_by_extension(&path) {
                    Some(false) => continue, // known binary
                    Some(true) => {}         // known text — dispatch
                    None => {}               // unknown — worker will check content
                }

                let path_str = path.to_string_lossy();
                if is_excluded(&path_str, exclude_list) {
                    continue;
                }

                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if job_tx.send((path, size)).is_err() {
                    return;
                }
            }
        }
    }
}

/// Perform an adaptive parallel search across the directory tree.
///
/// Uses whole-buffer `bytes::Regex` matching (rg-style): the regex engine's internal
/// SIMD/literal optimizations work on full file buffers rather than per-line.
/// Literal pre-filtering uses `memchr::memmem` (SIMD-accelerated substring search).
pub fn perform_search_adaptive(
    root_path: &str,
    pattern: &Regex,
    exclude_list: &[String],
    skip_hidden: bool,
    use_gitignore: bool,
) -> Result<Vec<SearchResult>, String> {
    let root = Path::new(root_path);
    if !root.exists() {
        return Err(format!("path does not exist: {}", root_path));
    }

    let bytes_pattern = BytesRegex::new(pattern.as_str())
        .map_err(|e| format!("regex compile error: {}", e))?;

    // Pre-build SIMD-accelerated literal finder (shared across all workers)
    let literal = extract_longest_literal(pattern.as_str());
    let finder: Option<memmem::Finder<'static>> =
        literal.map(|s| memmem::Finder::new(s.as_bytes()).into_owned());

    let num_workers = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let bytes_pattern = Arc::new(bytes_pattern);
    let finder = Arc::new(finder);

    let (job_tx, job_rx) = mpsc::channel::<(PathBuf, u64)>();
    let job_rx = Arc::new(Mutex::new(job_rx));

    // Spawn workers — each accumulates results locally (no result channel overhead)
    let mut handles = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        let job_rx = Arc::clone(&job_rx);
        let pattern = Arc::clone(&bytes_pattern);
        let finder = Arc::clone(&finder);

        handles.push(thread::spawn(move || {
            let mut local_results = Vec::new();
            loop {
                let job = {
                    let rx = job_rx.lock().unwrap();
                    rx.recv()
                };
                match job {
                    Ok((path, size)) => {
                        if size > MAX_FILE_SIZE {
                            continue;
                        }
                        let content = match fs::read(&path) {
                            Ok(c) => c,
                            Err(_) => continue,
                        };
                        // Fast binary detection: SIMD null-byte scan on first 512 bytes
                        if filedetect::is_binary_content(&content) {
                            continue;
                        }
                        let path_str = path.to_string_lossy().to_string();
                        let finder_ref = (*finder).as_ref();
                        let file_results =
                            search_file(&path_str, &content, &pattern, finder_ref);
                        local_results.extend(file_results);
                    }
                    Err(_) => break,
                }
            }
            local_results
        }));
    }

    // Walk and dispatch (pipelined — workers start processing immediately)
    walk_and_dispatch(root, exclude_list, skip_hidden, use_gitignore, &job_tx);
    drop(job_tx);

    // Collect results from all workers
    let mut all_results = Vec::new();
    for h in handles {
        if let Ok(results) = h.join() {
            all_results.extend(results);
        }
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
    fn test_extract_longest_literal_plain() {
        assert_eq!(
            extract_longest_literal("hello"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn test_extract_longest_literal_case_insensitive() {
        assert_eq!(extract_longest_literal("(?i)hello"), None);
    }

    #[test]
    fn test_extract_longest_literal_metachar() {
        // "he" and "lo" are both < 3 chars → None
        assert_eq!(extract_longest_literal("he.*lo"), None);
    }

    #[test]
    fn test_extract_longest_literal_escaped() {
        assert_eq!(
            extract_longest_literal("hel\\.lo"),
            Some("hel.lo".to_string())
        );
    }

    #[test]
    fn test_extract_longest_literal_short() {
        assert_eq!(
            extract_longest_literal("abc"),
            Some("abc".to_string())
        );
    }

    #[test]
    fn test_extract_longest_literal_too_short() {
        assert_eq!(extract_longest_literal("ab"), None);
    }

    #[test]
    fn test_extract_longest_literal_suffix() {
        assert_eq!(
            extract_longest_literal(".*important_function"),
            Some("important_function".to_string())
        );
    }

    #[test]
    fn test_extract_longest_literal_picks_longest() {
        assert_eq!(
            extract_longest_literal("abc.*defghij"),
            Some("defghij".to_string())
        );
    }
}
