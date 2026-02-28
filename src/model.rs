use regex::Regex;
use std::collections::HashSet;

/// A single search match within a file.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SearchResult {
    pub file_path: String,
    pub line_num: usize,
    pub line_text: String,
    pub match_text: String,
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
