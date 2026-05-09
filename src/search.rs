use memchr::memmem;
use regex::bytes::Regex as BytesRegex;
use regex::Regex;
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
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
const MAX_SEARCH_WORKERS: usize = 8;
const MAX_IN_FLIGHT_FILE_BYTES: usize = MAX_FILE_SIZE as usize;
const MAX_TOTAL_RESULT_BYTES: usize = 32 * 1024 * 1024;
const MAX_LINE_PREVIEW_BYTES: usize = 8 * 1024;

struct SearchBudget {
    max_in_flight_file_bytes: usize,
    max_total_result_bytes: usize,
    in_flight_file_bytes: Mutex<usize>,
    in_flight_file_cv: Condvar,
    total_result_bytes: AtomicUsize,
}

struct FileBudgetGuard<'a> {
    budget: &'a SearchBudget,
    bytes: usize,
}

impl SearchBudget {
    fn new(max_in_flight_file_bytes: usize, max_total_result_bytes: usize) -> Self {
        Self {
            max_in_flight_file_bytes,
            max_total_result_bytes,
            in_flight_file_bytes: Mutex::new(0),
            in_flight_file_cv: Condvar::new(),
            total_result_bytes: AtomicUsize::new(0),
        }
    }

    fn acquire_file_bytes(&self, bytes: usize) -> Result<FileBudgetGuard<'_>, String> {
        if bytes > self.max_in_flight_file_bytes {
            return Err(format!(
                "file exceeds in-flight search memory budget of {} MiB",
                self.max_in_flight_file_bytes / (1024 * 1024)
            ));
        }

        let mut used = self.in_flight_file_bytes.lock().unwrap();
        while used
            .checked_add(bytes)
            .map(|next| next > self.max_in_flight_file_bytes)
            .unwrap_or(true)
        {
            used = self.in_flight_file_cv.wait(used).unwrap();
        }
        *used += bytes;

        Ok(FileBudgetGuard {
            budget: self,
            bytes,
        })
    }

    fn reserve_result_bytes(&self, bytes: usize) -> Result<(), String> {
        loop {
            let current = self.total_result_bytes.load(Ordering::Relaxed);
            let next = current.checked_add(bytes).ok_or_else(|| {
                "search result set size overflowed internal accounting".to_string()
            })?;

            if next > self.max_total_result_bytes {
                return Err(format!(
                    "search result set exceeds {} MiB memory budget",
                    self.max_total_result_bytes / (1024 * 1024)
                ));
            }

            if self
                .total_result_bytes
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
}

impl Drop for FileBudgetGuard<'_> {
    fn drop(&mut self) {
        let mut used = self.budget.in_flight_file_bytes.lock().unwrap();
        *used = used.saturating_sub(self.bytes);
        self.budget.in_flight_file_cv.notify_one();
    }
}

fn build_line_preview(
    content: &[u8],
    line_start: usize,
    line_end: usize,
    match_start: usize,
    match_end: usize,
) -> String {
    let line_len = line_end.saturating_sub(line_start);
    if line_len <= MAX_LINE_PREVIEW_BYTES {
        return String::from_utf8_lossy(&content[line_start..line_end]).into_owned();
    }

    let match_len = match_end.saturating_sub(match_start).max(1);
    let mut preview_start = if match_len >= MAX_LINE_PREVIEW_BYTES {
        match_start
    } else {
        match_start.saturating_sub((MAX_LINE_PREVIEW_BYTES - match_len) / 2)
    };
    preview_start = preview_start.max(line_start);

    let max_start = line_end.saturating_sub(MAX_LINE_PREVIEW_BYTES);
    if preview_start > max_start {
        preview_start = max_start;
    }

    let preview_end = (preview_start + MAX_LINE_PREVIEW_BYTES).min(line_end);
    let mut preview = String::with_capacity((preview_end - preview_start).saturating_add(6));
    if preview_start > line_start {
        preview.push_str("...");
    }
    preview.push_str(&String::from_utf8_lossy(
        &content[preview_start..preview_end],
    ));
    if preview_end < line_end {
        preview.push_str("...");
    }
    preview
}

fn estimate_result_bytes(line_text: &str) -> usize {
    mem::size_of::<SearchResult>().saturating_add(line_text.len())
}

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
    path: Arc<PathBuf>,
    display_path: Arc<str>,
    content: &[u8],
    pattern: &BytesRegex,
    finder: Option<&memmem::Finder>,
    budget: &SearchBudget,
) -> Result<Vec<SearchResult>, String> {
    if content.is_empty() {
        return Ok(Vec::new());
    }

    // Whole-file literal quick-reject via SIMD-accelerated memmem
    if let Some(f) = finder {
        if f.find(content).is_none() {
            return Ok(Vec::new());
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

        let line_text = build_line_preview(content, line_start, line_end, m.start(), m.end());
        budget.reserve_result_bytes(estimate_result_bytes(&line_text))?;
        results.push(SearchResult::from_shared_path(
            Arc::clone(&path),
            Arc::clone(&display_path),
            running_line_num,
            line_text,
        ));
    }

    Ok(results)
}

/// Directories to always skip during walk (sorted for binary search).
const SKIP_DIRS: &[&str] = &[
    ".cache",
    ".git",
    ".hg",
    ".mypy_cache",
    ".next",
    ".nuxt",
    ".pytest_cache",
    ".svn",
    ".tox",
    ".venv",
    "__pycache__",
    "node_modules",
    "target",
    "venv",
];

/// Resolve the default hidden-file skipping behavior for a search root.
///
/// Hidden files are skipped by default outside Git repo roots. When the search
/// root contains a `.git` directory, hidden files and directories are included
/// by default so repo metadata such as `.github/` is searchable without
/// requiring `--hidden`. The `.git` directory itself is still skipped by
/// `SKIP_DIRS`.
pub fn default_skip_hidden(root_path: &str, include_hidden: bool) -> bool {
    !include_hidden && !Path::new(root_path).join(".git").is_dir()
}

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
/// Cheap path filtering: OsStr-based checks (hidden and SKIP_DIRS) run on
/// `entry.file_name()` before any full path allocation. Path-aware gitignore
/// rules are evaluated on `entry.path()` only after those checks pass. For files
/// with unknown extensions, content-based binary detection is deferred to the
/// worker threads (which already have the file in memory).
fn walk_and_dispatch(
    root: &Path,
    exclude_list: &[String],
    skip_hidden: bool,
    use_gitignore: bool,
    job_tx: &mpsc::Sender<(PathBuf, u64)>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let initial_ignore = if use_gitignore {
        let mut ig = gitignore::load_ancestor_gitignores(root)?;
        ig = ig.merge_dir(root)?;
        Arc::new(ig)
    } else {
        Arc::new(GitIgnore::empty())
    };

    let mut stack: Vec<(PathBuf, Arc<GitIgnore>)> = vec![(root.to_path_buf(), initial_ignore)];

    while let Some((dir, inherited_ignore)) = stack.pop() {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }

            // --- Phase 1: OsStr-only checks (no path allocation) ---
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if skip_hidden && name_str.starts_with('.') {
                continue;
            }
            #[cfg(windows)]
            if skip_hidden && is_hidden_windows(&entry) {
                continue;
            }

            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if ft.is_dir() {
                if SKIP_DIRS.binary_search(&name_str.as_ref()).is_ok() {
                    continue;
                }
                let path = entry.path();
                if inherited_ignore.is_ignored(&path, true) {
                    continue;
                }
                // Load ignore files from this child directory
                let child_ignore = if use_gitignore {
                    Arc::new(inherited_ignore.merge_dir(&path)?)
                } else {
                    inherited_ignore.clone()
                };
                stack.push((path, child_ignore));
            } else if ft.is_file() {
                let path = entry.path();
                if inherited_ignore.is_ignored(&path, false) {
                    continue;
                }

                // --- Phase 2: Extension-only classification (no file I/O) ---
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
                    return Ok(());
                }
            }
        }
    }

    Ok(())
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

    let bytes_pattern =
        BytesRegex::new(pattern.as_str()).map_err(|e| format!("regex compile error: {}", e))?;

    // Pre-build SIMD-accelerated literal finder (shared across all workers)
    let literal = extract_longest_literal(pattern.as_str());
    let finder: Option<memmem::Finder<'static>> =
        literal.map(|s| memmem::Finder::new(s.as_bytes()).into_owned());

    let num_workers = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, MAX_SEARCH_WORKERS);

    let bytes_pattern = Arc::new(bytes_pattern);
    let finder = Arc::new(finder);
    let budget = Arc::new(SearchBudget::new(
        MAX_IN_FLIGHT_FILE_BYTES,
        MAX_TOTAL_RESULT_BYTES,
    ));
    let stop = Arc::new(AtomicBool::new(false));

    let (job_tx, job_rx) = mpsc::channel::<(PathBuf, u64)>();
    let job_rx = Arc::new(Mutex::new(job_rx));

    // Spawn workers — each accumulates results locally (no result channel overhead)
    let mut handles = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        let job_rx = Arc::clone(&job_rx);
        let pattern = Arc::clone(&bytes_pattern);
        let finder = Arc::clone(&finder);
        let budget = Arc::clone(&budget);
        let stop = Arc::clone(&stop);

        handles.push(thread::spawn(
            move || -> Result<Vec<SearchResult>, String> {
                let mut local_results = Vec::new();
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }

                    let job = {
                        let rx = job_rx.lock().unwrap();
                        rx.recv()
                    };
                    match job {
                        Ok((path, size)) => {
                            if stop.load(Ordering::Relaxed) {
                                break;
                            }
                            if size > MAX_FILE_SIZE {
                                continue;
                            }

                            let reserved_bytes = match usize::try_from(size) {
                                Ok(bytes) => bytes,
                                Err(_) => continue,
                            };
                            let _file_budget = match budget.acquire_file_bytes(reserved_bytes) {
                                Ok(guard) => guard,
                                Err(e) => {
                                    stop.store(true, Ordering::Relaxed);
                                    return Err(e);
                                }
                            };

                            let content = match fs::read(&path) {
                                Ok(c) => c,
                                Err(_) => continue,
                            };
                            // Fast binary detection: SIMD null-byte scan on first 512 bytes
                            if filedetect::is_binary_content(&content) {
                                continue;
                            }

                            let path = Arc::new(path);
                            let display_path = SearchResult::display_path_for(path.as_path());
                            let finder_ref = (*finder).as_ref();
                            let file_results = match search_file(
                                path,
                                display_path,
                                &content,
                                &pattern,
                                finder_ref,
                                &budget,
                            ) {
                                Ok(results) => results,
                                Err(e) => {
                                    stop.store(true, Ordering::Relaxed);
                                    return Err(e);
                                }
                            };
                            local_results.extend(file_results);
                        }
                        Err(_) => break,
                    }
                }
                Ok(local_results)
            },
        ));
    }

    // Walk and dispatch (pipelined — workers start processing immediately)
    let dispatch_result = walk_and_dispatch(
        root,
        exclude_list,
        skip_hidden,
        use_gitignore,
        &job_tx,
        &stop,
    );
    drop(job_tx);

    let mut worker_error = None;

    // Collect results from all workers
    let mut all_results = Vec::new();
    for h in handles {
        match h.join() {
            Ok(Ok(results)) => all_results.extend(results),
            Ok(Err(e)) => {
                stop.store(true, Ordering::Relaxed);
                if worker_error.is_none() {
                    worker_error = Some(e);
                }
            }
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                if worker_error.is_none() {
                    worker_error = Some("search worker panicked".to_string());
                }
            }
        }
    }

    dispatch_result?;
    if let Some(e) = worker_error {
        return Err(e);
    }

    // Sort results by file path then line number for deterministic output
    all_results.sort_by(|a, b| {
        a.file_path_raw
            .cmp(&b.file_path_raw)
            .then(a.line_num.cmp(&b.line_num))
    });

    Ok(all_results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_longest_literal_plain() {
        assert_eq!(extract_longest_literal("hello"), Some("hello".to_string()));
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
        assert_eq!(extract_longest_literal("abc"), Some("abc".to_string()));
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
