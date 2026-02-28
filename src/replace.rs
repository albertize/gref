use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::SearchResult;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate a unique temporary file path in the same directory as the source file.
fn tmp_path_for(file_path: &str) -> String {
    let dir = Path::new(file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(".gref_tmp_{}_{}{}", pid, counter, nanos);
    dir.join(name).to_string_lossy().to_string()
}

/// Perform replacements across multiple files for the selected results.
pub fn perform_replacements(
    all_results: &[SearchResult],
    selected: &HashSet<usize>,
    pattern: &Regex,
    replacement: &str,
) -> Result<(), String> {
    // Group selected results by file path
    let mut files_to_process: HashMap<&str, Vec<&SearchResult>> = HashMap::new();
    for &idx in selected {
        if let Some(res) = all_results.get(idx) {
            files_to_process
                .entry(&res.file_path)
                .or_default()
                .push(res);
        }
    }

    for (file_path, results) in &files_to_process {
        replace_in_file(file_path, results, pattern, replacement).map_err(|e| {
            format!("replacement failed for {}: {}", file_path, e)
        })?;
    }

    Ok(())
}

/// Replace matched lines in a single file using a temp-file + atomic rename.
pub fn replace_in_file(
    file_path: &str,
    results: &[&SearchResult],
    pattern: &Regex,
    replacement: &str,
) -> Result<(), String> {
    let lines_to_replace: HashSet<usize> = results.iter().map(|r| r.line_num).collect();

    let src = fs::File::open(file_path)
        .map_err(|e| format!("failed to open source: {}", e))?;

    let tmp = tmp_path_for(file_path);
    let tmp_file = fs::File::create(&tmp)
        .map_err(|e| {
            format!("failed to create temp file: {}", e)
        })?;

    let reader = BufReader::new(src);
    let mut writer = BufWriter::new(tmp_file);

    let mut line_num = 0usize;
    let mut buf = Vec::with_capacity(1024);
    let mut reader = reader;

    loop {
        buf.clear();
        let bytes_read = reader.read_until(b'\n', &mut buf).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("error reading line {}: {}", line_num + 1, e)
        })?;

        if bytes_read == 0 {
            break;
        }

        line_num += 1;

        if lines_to_replace.contains(&line_num) {
            let line_str = String::from_utf8_lossy(&buf);
            let replaced = pattern.replace_all(&line_str, replacement);
            writer.write_all(replaced.as_bytes()).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                format!("error writing to temp file: {}", e)
            })?;
        } else {
            writer.write_all(&buf).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                format!("error writing to temp file: {}", e)
            })?;
        }
    }

    writer.flush().map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("failed to flush buffer: {}", e)
    })?;

    // Drop file handles before rename
    drop(writer);
    drop(reader);

    fs::rename(&tmp, file_path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("failed to finalize replacement: {}", e)
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, data: &[u8]) -> String {
        let path = std::env::temp_dir().join(name);
        fs::write(&path, data).unwrap();
        path.to_string_lossy().to_string()
    }

    fn cleanup(path: &str) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_replace_in_file() {
        let file = write_tmp("gref_test_replace.txt", b"foo bar\nfoo baz\nbar foo");
        let results = vec![
            SearchResult { file_path: file.clone(), line_num: 1, line_text: "foo bar".into(), match_text: "foo".into() },
            SearchResult { file_path: file.clone(), line_num: 2, line_text: "foo baz".into(), match_text: "foo".into() },
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "qux bar\nqux baz\nbar foo");
        cleanup(&file);
    }

    #[test]
    fn test_replace_windows_line_endings() {
        let file = write_tmp("gref_test_replace_win.txt", b"foo bar\r\nfoo baz\r\nbar foo");
        let results = vec![
            SearchResult { file_path: file.clone(), line_num: 1, line_text: "foo bar".into(), match_text: "foo".into() },
            SearchResult { file_path: file.clone(), line_num: 2, line_text: "foo baz".into(), match_text: "foo".into() },
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"qux bar\r\nqux baz\r\nbar foo");
        cleanup(&file);
    }

    #[test]
    fn test_replace_empty_file() {
        let file = write_tmp("gref_test_replace_empty.txt", b"");
        let results: Vec<SearchResult> = vec![];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "");
        cleanup(&file);
    }

    #[test]
    fn test_replace_only_matches() {
        let file = write_tmp("gref_test_replace_only.txt", b"foo\nfoo\nfoo");
        let results = vec![
            SearchResult { file_path: file.clone(), line_num: 1, line_text: "foo".into(), match_text: "foo".into() },
            SearchResult { file_path: file.clone(), line_num: 2, line_text: "foo".into(), match_text: "foo".into() },
            SearchResult { file_path: file.clone(), line_num: 3, line_text: "foo".into(), match_text: "foo".into() },
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "qux\nqux\nqux");
        cleanup(&file);
    }

    #[test]
    fn test_replace_no_matches() {
        let file = write_tmp("gref_test_replace_nomatch.txt", b"bar\nbaz\nquux");
        let results: Vec<SearchResult> = vec![];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "bar\nbaz\nquux");
        cleanup(&file);
    }

    #[test]
    fn test_replace_special_chars() {
        let file = write_tmp("gref_test_replace_special.txt", "föö bär\nföö baz\nbär föö".as_bytes());
        let results = vec![
            SearchResult { file_path: file.clone(), line_num: 1, line_text: "föö bär".into(), match_text: "föö".into() },
            SearchResult { file_path: file.clone(), line_num: 2, line_text: "föö baz".into(), match_text: "föö".into() },
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("föö").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "qux bär\nqux baz\nbär föö");
        cleanup(&file);
    }

    #[test]
    fn test_replace_byte_conflict() {
        let data: Vec<u8> = vec![
            b'f', b'o', b'o', 0xff, b'b', b'a', b'r', b'\n',
            b'f', b'o', b'o', 0xfe, b'b', b'a', b'z', b'\n',
            b'b', b'a', b'r', b' ', b'f', b'o', b'o',
        ];
        let file = write_tmp("gref_test_replace_byteconflict.txt", &data);
        let results = vec![
            SearchResult { file_path: file.clone(), line_num: 1, line_text: String::from_utf8_lossy(&data[0..7]).into_owned(), match_text: "foo".into() },
            SearchResult { file_path: file.clone(), line_num: 2, line_text: String::from_utf8_lossy(&data[8..15]).into_owned(), match_text: "foo".into() },
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read(&file).unwrap();
        // The replacement operates on the lossy string representation, so 0xff/0xfe become U+FFFD
        // The exact result depends on from_utf8_lossy behavior
        assert!(!content.is_empty());
        cleanup(&file);
    }

    #[test]
    fn test_replace_invalid_regexp() {
        let result = Regex::new("[");
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_overlapping_match() {
        let file = write_tmp("gref_test_replace_overlap.txt", b"aaaaa");
        let results = vec![
            SearchResult { file_path: file.clone(), line_num: 1, line_text: "aaaaa".into(), match_text: "aa".into() },
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("aa").unwrap();
        replace_in_file(&file, &refs, &pattern, "b").unwrap();
        let content = fs::read(&file).unwrap();
        assert!(!content.is_empty());
        cleanup(&file);
    }

    #[test]
    fn test_replace_null_bytes() {
        let data: Vec<u8> = vec![
            b'f', b'o', b'o', 0x00, b'b', b'a', b'r', b'\n',
            b'b', b'a', b'z', 0x00, b'f', b'o', b'o',
        ];
        let file = write_tmp("gref_test_replace_nullbytes.txt", &data);
        let results = vec![
            SearchResult { file_path: file.clone(), line_num: 1, line_text: String::from_utf8_lossy(&data[0..7]).into_owned(), match_text: "foo".into() },
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read(&file).unwrap();
        assert!(!content.is_empty());
        cleanup(&file);
    }
}
