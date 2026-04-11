use regex::Regex;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

fn push_escaped_display_text(out: &mut String, text: &str, escape_backslash: bool) {
    for ch in text.chars() {
        match ch {
            '\\' if escape_backslash => out.push_str("\\\\"),
            c if c.is_control() => {
                let mut buf = [0u8; 4];
                for byte in c.encode_utf8(&mut buf).as_bytes() {
                    let _ = write!(out, "\\x{:02X}", byte);
                }
            }
            _ => out.push(ch),
        }
    }
}

#[cfg(unix)]
fn display_path_string(path: &Path) -> String {
    let mut out = String::new();
    let mut remaining = path.as_os_str().as_bytes();

    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(valid) => {
                push_escaped_display_text(&mut out, valid, true);
                break;
            }
            Err(err) => {
                let valid_up_to = err.valid_up_to();
                if valid_up_to > 0 {
                    push_escaped_display_text(
                        &mut out,
                        std::str::from_utf8(&remaining[..valid_up_to]).unwrap_or_default(),
                        true,
                    );
                }

                let invalid_len = err.error_len().unwrap_or(1);
                for &byte in &remaining[valid_up_to..valid_up_to + invalid_len] {
                    let _ = write!(out, "\\x{:02X}", byte);
                }

                remaining = &remaining[valid_up_to + invalid_len..];
            }
        }
    }

    out
}

#[cfg(not(unix))]
fn display_path_string(path: &Path) -> String {
    let mut out = String::new();
    let lossy = path.to_string_lossy();
    push_escaped_display_text(&mut out, &lossy, false);
    out
}

/// A single search match within a file.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SearchResult {
    pub file_path: Arc<str>,
    pub file_path_raw: Arc<PathBuf>,
    pub line_num: usize,
    pub line_text: String,
}

impl SearchResult {
    pub fn from_path(file_path: PathBuf, line_num: usize, line_text: impl Into<String>) -> Self {
        let file_path_raw = Arc::new(file_path);
        Self {
            file_path: Self::display_path_for(file_path_raw.as_path()),
            file_path_raw,
            line_num,
            line_text: line_text.into(),
        }
    }

    pub fn from_display_path(
        file_path: impl Into<String>,
        line_num: usize,
        line_text: impl Into<String>,
    ) -> Self {
        let file_path = file_path.into();
        Self {
            file_path: Arc::<str>::from(file_path.clone()),
            file_path_raw: Arc::new(PathBuf::from(file_path)),
            line_num,
            line_text: line_text.into(),
        }
    }

    pub fn from_shared_path(
        file_path_raw: Arc<PathBuf>,
        file_path: Arc<str>,
        line_num: usize,
        line_text: impl Into<String>,
    ) -> Self {
        Self {
            file_path,
            file_path_raw,
            line_num,
            line_text: line_text.into(),
        }
    }

    pub fn display_path_for(file_path: &Path) -> Arc<str> {
        Arc::<str>::from(display_path_string(file_path))
    }

    pub fn path(&self) -> &Path {
        self.file_path_raw.as_path()
    }
}

/// The current UI state of the application.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    #[default]
    Browse,
    Confirming,
    Replacing,
    Done,
}

/// The mode the application is operating in.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    #[default]
    Default,
    SearchOnly,
}

/// Holds the full state of the TUI application.
pub struct Model {
    pub results: Vec<SearchResult>,
    pub cursor: usize,
    pub topline: usize,
    pub screen_height: usize,
    pub screen_width: usize,
    pub selected: HashSet<usize>,
    pub pattern: Regex,
    pub pattern_str: String,
    pub replacement_str: String,
    pub mode: AppMode,
    pub state: AppState,
    pub error: Option<String>,
    pub horizontal_offset: usize,
}

impl Model {
    pub fn new(
        results: Vec<SearchResult>,
        pattern_str: String,
        replacement_str: String,
        pattern: Regex,
        mode: AppMode,
    ) -> Self {
        Model {
            results,
            cursor: 0,
            topline: 0,
            screen_height: 20,
            screen_width: 80,
            selected: HashSet::new(),
            pattern,
            pattern_str,
            replacement_str,
            mode,
            state: AppState::Browse,
            error: None,
            horizontal_offset: 0,
        }
    }
}
