use regex::bytes::Regex as BytesRegex;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::SearchResult;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
const MAX_REPLACE_LINE_BYTES: usize = 64 * 1024 * 1024;

/// Generate a unique temporary file path in the same directory as the source file.
fn tmp_path_for(file_path: &Path) -> PathBuf {
    let dir = file_path.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(".gref_tmp_{}_{}{}", pid, counter, nanos);
    dir.join(name)
}

fn compile_bytes_pattern(pattern: &Regex) -> Result<BytesRegex, String> {
    BytesRegex::new(pattern.as_str())
        .map_err(|e| format!("failed to compile replacement regex: {}", e))
}

enum CaptureRef<'a> {
    Index(usize),
    Name(&'a str),
}

fn parse_capture_ref(replacement: &[u8]) -> Option<(usize, CaptureRef<'_>)> {
    if replacement.len() <= 1 || replacement[0] != b'$' {
        return None;
    }

    if replacement[1] == b'{' {
        let end = replacement[2..].iter().position(|&b| b == b'}')? + 2;
        let name = std::str::from_utf8(&replacement[2..end]).ok()?;
        let cap = match name.parse::<usize>() {
            Ok(index) => CaptureRef::Index(index),
            Err(_) => CaptureRef::Name(name),
        };
        return Some((end + 1, cap));
    }

    let mut end = 1usize;
    while replacement
        .get(end)
        .copied()
        .map(|b| b.is_ascii_alphanumeric() || b == b'_')
        .unwrap_or(false)
    {
        end += 1;
    }
    if end == 1 {
        return None;
    }

    let name = std::str::from_utf8(&replacement[1..end]).ok()?;
    let cap = match name.parse::<usize>() {
        Ok(index) => CaptureRef::Index(index),
        Err(_) => CaptureRef::Name(name),
    };
    Some((end, cap))
}

fn write_capture<W: Write>(
    writer: &mut W,
    caps: &regex::bytes::Captures<'_>,
    cap_ref: CaptureRef<'_>,
) -> io::Result<()> {
    let matched = match cap_ref {
        CaptureRef::Index(index) => caps.get(index),
        CaptureRef::Name(name) => caps.name(name),
    };
    if let Some(m) = matched {
        writer.write_all(m.as_bytes())?;
    }
    Ok(())
}

fn write_expanded_replacement<W: Write>(
    writer: &mut W,
    replacement: &[u8],
    caps: &regex::bytes::Captures<'_>,
) -> io::Result<()> {
    let mut offset = 0usize;

    while let Some(rel_dollar) = replacement[offset..].iter().position(|&b| b == b'$') {
        let dollar = offset + rel_dollar;
        writer.write_all(&replacement[offset..dollar])?;

        if replacement.get(dollar + 1) == Some(&b'$') {
            writer.write_all(b"$")?;
            offset = dollar + 2;
            continue;
        }

        if let Some((end, cap_ref)) = parse_capture_ref(&replacement[dollar..]) {
            write_capture(writer, caps, cap_ref)?;
            offset = dollar + end;
            continue;
        }

        writer.write_all(b"$")?;
        offset = dollar + 1;
    }

    writer.write_all(&replacement[offset..])
}

fn append_selected_bytes(
    selected_line: &mut Vec<u8>,
    bytes: &[u8],
    line_num: usize,
    max_line_bytes: usize,
) -> Result<(), String> {
    let new_len = selected_line
        .len()
        .checked_add(bytes.len())
        .ok_or_else(|| format!("line {} exceeds maximum replaceable line size", line_num))?;

    if new_len > max_line_bytes {
        return Err(format!(
            "line {} exceeds maximum replaceable line size of {} bytes",
            line_num, max_line_bytes
        ));
    }

    selected_line.extend_from_slice(bytes);
    Ok(())
}

// Stream replacements directly to the output file so selected lines don't need a
// second full-size allocation, while preserving arbitrary bytes outside matches.
fn write_replaced_line<W: Write>(
    writer: &mut W,
    line: &[u8],
    pattern: &BytesRegex,
    replacement: &[u8],
    expand_captures: bool,
) -> io::Result<()> {
    let mut last_end = 0usize;

    if expand_captures && replacement.contains(&b'$') {
        for caps in pattern.captures_iter(line) {
            let Some(m) = caps.get(0) else {
                continue;
            };
            writer.write_all(&line[last_end..m.start()])?;
            write_expanded_replacement(writer, replacement, &caps)?;
            last_end = m.end();
        }
    } else {
        for m in pattern.find_iter(line) {
            writer.write_all(&line[last_end..m.start()])?;
            writer.write_all(replacement)?;
            last_end = m.end();
        }
    }

    writer.write_all(&line[last_end..])
}

/// Perform replacements across multiple files for the selected results.
pub fn perform_replacements(
    all_results: &[SearchResult],
    selected: &HashSet<usize>,
    pattern: &Regex,
    replacement: &str,
) -> Result<(), String> {
    perform_replacements_with_options(all_results, selected, pattern, replacement, true)
}

/// Perform replacements with explicit replacement expansion control.
pub fn perform_replacements_with_options(
    all_results: &[SearchResult],
    selected: &HashSet<usize>,
    pattern: &Regex,
    replacement: &str,
    expand_captures: bool,
) -> Result<(), String> {
    let bytes_pattern = compile_bytes_pattern(pattern)?;

    // Group selected results by file path
    let mut files_to_process: HashMap<&Path, Vec<&SearchResult>> = HashMap::new();
    for &idx in selected {
        if let Some(res) = all_results.get(idx) {
            files_to_process.entry(res.path()).or_default().push(res);
        }
    }

    for (file_path, results) in &files_to_process {
        replace_in_path_bytes(
            file_path,
            results,
            &bytes_pattern,
            replacement.as_bytes(),
            expand_captures,
        )
        .map_err(|e| format!("replacement failed for {}: {}", file_path.display(), e))?;
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
    replace_in_file_with_options(file_path, results, pattern, replacement, true)
}

/// Replace matched lines in a single file with explicit replacement expansion control.
pub fn replace_in_file_with_options(
    file_path: &str,
    results: &[&SearchResult],
    pattern: &Regex,
    replacement: &str,
    expand_captures: bool,
) -> Result<(), String> {
    let bytes_pattern = compile_bytes_pattern(pattern)?;
    replace_in_path_bytes(
        Path::new(file_path),
        results,
        &bytes_pattern,
        replacement.as_bytes(),
        expand_captures,
    )
}

fn replace_in_path_bytes(
    file_path: &Path,
    results: &[&SearchResult],
    pattern: &BytesRegex,
    replacement: &[u8],
    expand_captures: bool,
) -> Result<(), String> {
    replace_in_path_bytes_with_limit(
        file_path,
        results,
        pattern,
        replacement,
        expand_captures,
        MAX_REPLACE_LINE_BYTES,
    )
}

fn replace_in_path_bytes_with_limit(
    file_path: &Path,
    results: &[&SearchResult],
    pattern: &BytesRegex,
    replacement: &[u8],
    expand_captures: bool,
    max_line_bytes: usize,
) -> Result<(), String> {
    let lines_to_replace: HashSet<usize> = results.iter().map(|r| r.line_num).collect();

    let src = fs::File::open(file_path).map_err(|e| format!("failed to open source: {}", e))?;

    let tmp = tmp_path_for(file_path);
    let tmp_file =
        fs::File::create(&tmp).map_err(|e| format!("failed to create temp file: {}", e))?;

    let mut reader = BufReader::new(src);
    let mut writer = BufWriter::new(tmp_file);

    let mut current_line_num = 1usize;
    let mut current_line_selected = lines_to_replace.contains(&current_line_num);
    let mut selected_line = Vec::with_capacity(1024);

    loop {
        let consumed = {
            let available = reader.fill_buf().map_err(|e| {
                let _ = fs::remove_file(&tmp);
                format!("error reading line {}: {}", current_line_num, e)
            })?;

            if available.is_empty() {
                0
            } else {
                let mut offset = 0usize;
                while offset < available.len() {
                    if let Some(rel_newline) = memchr::memchr(b'\n', &available[offset..]) {
                        let end = offset + rel_newline + 1;
                        let chunk = &available[offset..end];

                        if current_line_selected {
                            append_selected_bytes(
                                &mut selected_line,
                                chunk,
                                current_line_num,
                                max_line_bytes,
                            )
                            .inspect_err(|_| {
                                let _ = fs::remove_file(&tmp);
                            })?;
                            write_replaced_line(
                                &mut writer,
                                &selected_line,
                                pattern,
                                replacement,
                                expand_captures,
                            )
                            .map_err(|e| {
                                let _ = fs::remove_file(&tmp);
                                format!("error writing to temp file: {}", e)
                            })?;
                            selected_line.clear();
                        } else {
                            writer.write_all(chunk).map_err(|e| {
                                let _ = fs::remove_file(&tmp);
                                format!("error writing to temp file: {}", e)
                            })?;
                        }

                        current_line_num += 1;
                        current_line_selected = lines_to_replace.contains(&current_line_num);
                        offset = end;
                    } else {
                        let chunk = &available[offset..];
                        if current_line_selected {
                            append_selected_bytes(
                                &mut selected_line,
                                chunk,
                                current_line_num,
                                max_line_bytes,
                            )
                            .inspect_err(|_| {
                                let _ = fs::remove_file(&tmp);
                            })?;
                        } else {
                            writer.write_all(chunk).map_err(|e| {
                                let _ = fs::remove_file(&tmp);
                                format!("error writing to temp file: {}", e)
                            })?;
                        }
                        offset = available.len();
                    }
                }

                available.len()
            }
        };

        if consumed == 0 {
            break;
        }

        reader.consume(consumed);
    }

    if current_line_selected && !selected_line.is_empty() {
        write_replaced_line(
            &mut writer,
            &selected_line,
            pattern,
            replacement,
            expand_captures,
        )
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("error writing to temp file: {}", e)
        })?;
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
        let results = [
            SearchResult::from_display_path(&file, 1, "foo bar"),
            SearchResult::from_display_path(&file, 2, "foo baz"),
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
        let file = write_tmp(
            "gref_test_replace_win.txt",
            b"foo bar\r\nfoo baz\r\nbar foo",
        );
        let results = [
            SearchResult::from_display_path(&file, 1, "foo bar"),
            SearchResult::from_display_path(&file, 2, "foo baz"),
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
        let results: [SearchResult; 0] = [];
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
        let results = [
            SearchResult::from_display_path(&file, 1, "foo"),
            SearchResult::from_display_path(&file, 2, "foo"),
            SearchResult::from_display_path(&file, 3, "foo"),
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
        let results: [SearchResult; 0] = [];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "bar\nbaz\nquux");
        cleanup(&file);
    }

    #[test]
    fn test_replace_special_chars() {
        let file = write_tmp(
            "gref_test_replace_special.txt",
            "föö bär\nföö baz\nbär föö".as_bytes(),
        );
        let results = [
            SearchResult::from_display_path(&file, 1, "föö bär"),
            SearchResult::from_display_path(&file, 2, "föö baz"),
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
            b'f', b'o', b'o', 0xff, b'b', b'a', b'r', b'\n', b'f', b'o', b'o', 0xfe, b'b', b'a',
            b'z', b'\n', b'b', b'a', b'r', b' ', b'f', b'o', b'o',
        ];
        let file = write_tmp("gref_test_replace_byteconflict.txt", &data);
        let results = [
            SearchResult::from_display_path(
                &file,
                1,
                String::from_utf8_lossy(&data[0..7]).into_owned(),
            ),
            SearchResult::from_display_path(
                &file,
                2,
                String::from_utf8_lossy(&data[8..15]).into_owned(),
            ),
        ];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read(&file).unwrap();
        assert_eq!(
            content,
            vec![
                b'q', b'u', b'x', 0xff, b'b', b'a', b'r', b'\n', b'q', b'u', b'x', 0xfe, b'b',
                b'a', b'z', b'\n', b'b', b'a', b'r', b' ', b'f', b'o', b'o',
            ]
        );
        cleanup(&file);
    }

    #[test]
    fn test_replace_capture_expansion() {
        let file = write_tmp("gref_test_replace_capture.txt", b"foo-123\nbar\n");
        let results = [SearchResult::from_display_path(&file, 1, "foo-123")];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new(r"(foo)-(\d+)").unwrap();
        replace_in_file(&file, &refs, &pattern, "$2:$1").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "123:foo\nbar\n");
        cleanup(&file);
    }

    #[test]
    fn test_replace_capture_expansion_with_literal_dollar() {
        let file = write_tmp("gref_test_replace_capture_dollar.txt", b"foo-123\n");
        let results = [SearchResult::from_display_path(&file, 1, "foo-123")];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new(r"(foo)-(\d+)").unwrap();
        replace_in_file(&file, &refs, &pattern, "$$$2").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "$123\n");
        cleanup(&file);
    }

    #[test]
    fn test_replace_capture_longest_name_parse() {
        let file = write_tmp("gref_test_replace_capture_name_parse.txt", b"foo-123\n");
        let results = [SearchResult::from_display_path(&file, 1, "foo-123")];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new(r"(foo)-(\d+)").unwrap();
        replace_in_file(&file, &refs, &pattern, "$1_$2").unwrap();
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "123\n");
        cleanup(&file);
    }

    #[test]
    fn test_replace_invalid_regexp() {
        let invalid = String::from("[");
        let result = Regex::new(&invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_overlapping_match() {
        let file = write_tmp("gref_test_replace_overlap.txt", b"aaaaa");
        let results = [SearchResult::from_display_path(&file, 1, "aaaaa")];
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
            b'f', b'o', b'o', 0x00, b'b', b'a', b'r', b'\n', b'b', b'a', b'z', 0x00, b'f', b'o',
            b'o',
        ];
        let file = write_tmp("gref_test_replace_nullbytes.txt", &data);
        let results = [SearchResult::from_display_path(
            &file,
            1,
            String::from_utf8_lossy(&data[0..7]).into_owned(),
        )];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = Regex::new("foo").unwrap();
        replace_in_file(&file, &refs, &pattern, "qux").unwrap();
        let content = fs::read(&file).unwrap();
        assert_eq!(
            content,
            vec![
                b'q', b'u', b'x', 0x00, b'b', b'a', b'r', b'\n', b'b', b'a', b'z', 0x00, b'f',
                b'o', b'o',
            ]
        );
        cleanup(&file);
    }

    #[test]
    fn test_replace_selected_line_too_large_returns_error() {
        let dir = std::env::temp_dir().join("gref_test_replace_too_large");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("input.txt");
        let data = b"keep\nfoofoofoo\nlast\n";
        fs::write(&file, data).unwrap();

        let file_str = file.to_string_lossy().to_string();
        let results = [SearchResult::from_display_path(&file_str, 2, "foofoofoo")];
        let refs: Vec<&SearchResult> = results.iter().collect();
        let pattern = compile_bytes_pattern(&Regex::new("foo").unwrap()).unwrap();

        let err = replace_in_path_bytes_with_limit(
            Path::new(&file_str),
            &refs,
            &pattern,
            b"bar",
            true,
            4,
        )
        .unwrap_err();

        assert!(err.contains("maximum replaceable line size"));
        assert_eq!(fs::read(&file).unwrap(), data);

        let entries: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["input.txt".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }
}
