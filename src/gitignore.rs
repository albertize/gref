use regex::Regex;
use std::fs;
use std::path::Path;

#[derive(Clone)]
struct IgnoreRule {
    regex: Regex,
    is_negation: bool,
    dir_only: bool,
}

#[derive(Clone)]
pub struct GitIgnore {
    rules: Vec<IgnoreRule>,
}

/// Convert a gitignore glob pattern to a regex string.
///
/// Supports: `*` (any non-slash), `**` (any including slash), `?` (single non-slash),
/// `[...]` (character class), `\` (escape).
fn glob_to_regex(glob: &str) -> String {
    let mut regex = String::with_capacity(glob.len() * 2);
    regex.push('^');

    let chars: Vec<char> = glob.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    i += 1;
                    if i + 1 < chars.len() && chars[i + 1] == '/' {
                        i += 1;
                        regex.push_str("(.+/)?");
                    } else {
                        regex.push_str(".*");
                    }
                } else {
                    regex.push_str("[^/]*");
                }
            }
            '?' => regex.push_str("[^/]"),
            '.' => regex.push_str(r"\."),
            '+' | '(' | ')' | '{' | '}' | '|' | '^' | '$' => {
                regex.push('\\');
                regex.push(chars[i]);
            }
            '[' => {
                regex.push('[');
                i += 1;
                while i < chars.len() {
                    if chars[i] == ']' {
                        regex.push(']');
                        break;
                    }
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        regex.push('\\');
                        i += 1;
                        regex.push(chars[i]);
                    } else {
                        regex.push(chars[i]);
                    }
                    i += 1;
                }
            }
            '\\' => {
                i += 1;
                if i < chars.len() {
                    let c = chars[i];
                    if ".+*?^$()[]{}|\\".contains(c) {
                        regex.push('\\');
                    }
                    regex.push(c);
                }
            }
            _ => regex.push(chars[i]),
        }
        i += 1;
    }

    regex.push('$');
    regex
}

/// Parse a single .gitignore line into an IgnoreRule.
///
/// Supports comments (`#`), negation (`!`), directory-only (`/` suffix),
/// anchored patterns (`/` prefix), and basename-only matching.
/// Path patterns (containing `/` in the middle) are skipped — only basename matching is used.
fn parse_line(line: &str) -> Option<IgnoreRule> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let mut s = line;

    let is_negation = s.starts_with('!');
    if is_negation {
        s = &s[1..];
    }

    let dir_only = s.ends_with('/');
    if dir_only {
        s = &s[..s.len() - 1];
    }

    // Strip leading / (anchored pattern — treated as basename for simplicity)
    let s = s.strip_prefix('/').unwrap_or(s);

    // Skip path patterns (contain / in middle) — we only do basename matching
    if s.contains('/') {
        return None;
    }

    if s.is_empty() {
        return None;
    }

    let regex_str = glob_to_regex(s);
    let regex = Regex::new(&regex_str).ok()?;

    Some(IgnoreRule {
        regex,
        is_negation,
        dir_only,
    })
}

impl GitIgnore {
    pub fn empty() -> Self {
        GitIgnore { rules: Vec::new() }
    }

    pub fn from_path(path: &Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::empty(),
        };
        let rules = content.lines().filter_map(parse_line).collect();
        GitIgnore { rules }
    }

    /// Merge with rules from a child ignore file.
    /// Parent rules come first, child rules last (last match wins).
    pub fn merge_file(&self, path: &Path) -> GitIgnore {
        let child = GitIgnore::from_path(path);
        if child.rules.is_empty() {
            return self.clone();
        }
        let mut rules = self.rules.clone();
        rules.extend(child.rules);
        GitIgnore { rules }
    }

    /// Merge all ignore files (.gitignore, .ignore, .grefignore) from a directory.
    /// Priority (last match wins): .gitignore < .ignore < .grefignore.
    pub fn merge_dir(&self, dir: &Path) -> GitIgnore {
        const IGNORE_FILES: &[&str] = &[".gitignore", ".ignore", ".grefignore"];
        let mut result = self.clone();
        for name in IGNORE_FILES {
            let path = dir.join(name);
            if path.is_file() {
                result = result.merge_file(&path);
            }
        }
        result
    }

    /// Check whether a name should be ignored. Last matching rule wins.
    pub fn is_ignored(&self, name: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for rule in &self.rules {
            if rule.dir_only && !is_dir {
                continue;
            }
            if rule.regex.is_match(name) {
                ignored = !rule.is_negation;
            }
        }
        ignored
    }
}

/// Ignore file names loaded per directory (in priority order).
const IGNORE_FILES: &[&str] = &[".gitignore", ".ignore", ".grefignore"];

/// Load ignore files from ancestor directories up to the repository root.
///
/// Walks up from `root`'s parent until a `.git` directory is found.
/// Loads `.gitignore`, `.ignore`, and `.grefignore` at each level.
/// Returns merged rules applied from repo root down to the search root.
pub fn load_ancestor_gitignores(root: &Path) -> GitIgnore {
    let mut ancestor_dirs = Vec::new();
    let mut current = match root.parent() {
        Some(p) => p.to_path_buf(),
        None => return GitIgnore::empty(),
    };

    loop {
        ancestor_dirs.push(current.clone());
        if current.join(".git").exists() {
            break;
        }
        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => break,
        }
    }

    // Apply from repo root (last) down to closest ancestor (first)
    ancestor_dirs.reverse();
    let mut result = GitIgnore::empty();
    for dir in &ancestor_dirs {
        for name in IGNORE_FILES {
            let path = dir.join(name);
            if path.is_file() {
                result = result.merge_file(&path);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_to_regex_simple() {
        assert_eq!(glob_to_regex("*.pyc"), r"^[^/]*\.pyc$");
        assert_eq!(glob_to_regex("node_modules"), "^node_modules$");
    }

    #[test]
    fn test_glob_to_regex_doublestar() {
        assert_eq!(glob_to_regex("**/*.log"), r"^(.+/)?[^/]*\.log$");
    }

    #[test]
    fn test_glob_to_regex_question() {
        assert_eq!(glob_to_regex("file?.txt"), r"^file[^/]\.txt$");
    }

    #[test]
    fn test_glob_to_regex_char_class() {
        assert_eq!(glob_to_regex("*.py[cod]"), r"^[^/]*\.py[cod]$");
    }

    #[test]
    fn test_parse_line_comment() {
        assert!(parse_line("# comment").is_none());
        assert!(parse_line("  ").is_none());
        assert!(parse_line("").is_none());
    }

    #[test]
    fn test_parse_line_simple() {
        let rule = parse_line("*.pyc").unwrap();
        assert!(!rule.is_negation);
        assert!(!rule.dir_only);
        assert!(rule.regex.is_match("foo.pyc"));
        assert!(!rule.regex.is_match("foo.py"));
    }

    #[test]
    fn test_parse_line_dir_only() {
        let rule = parse_line("build/").unwrap();
        assert!(rule.dir_only);
        assert!(rule.regex.is_match("build"));
    }

    #[test]
    fn test_parse_line_negation() {
        let rule = parse_line("!important.log").unwrap();
        assert!(rule.is_negation);
        assert!(rule.regex.is_match("important.log"));
    }

    #[test]
    fn test_parse_line_leading_slash() {
        let rule = parse_line("/target").unwrap();
        assert!(rule.regex.is_match("target"));
    }

    #[test]
    fn test_parse_line_path_pattern_skipped() {
        assert!(parse_line("docs/build").is_none());
        assert!(parse_line("src/generated/").is_none());
    }

    #[test]
    fn test_gitignore_is_ignored() {
        let gi = GitIgnore {
            rules: vec![
                parse_line("*.pyc").unwrap(),
                parse_line("build/").unwrap(),
                parse_line("*.log").unwrap(),
                parse_line("!important.log").unwrap(),
            ],
        };
        assert!(gi.is_ignored("foo.pyc", false));
        assert!(gi.is_ignored("build", true));
        assert!(!gi.is_ignored("build", false)); // dir_only
        assert!(gi.is_ignored("debug.log", false));
        assert!(!gi.is_ignored("important.log", false)); // negated
        assert!(!gi.is_ignored("main.rs", false));
    }

    #[test]
    fn test_gitignore_empty() {
        let gi = GitIgnore::empty();
        assert!(!gi.is_ignored("anything", false));
    }

    #[test]
    fn test_gitignore_from_file() {
        let dir = std::env::temp_dir().join("gref_test_gitignore");
        let _ = fs::create_dir_all(&dir);
        fs::write(
            dir.join(".gitignore"),
            "# Comment\n*.pyc\nbuild/\n!keep.pyc\n",
        )
        .unwrap();

        let gi = GitIgnore::from_path(&dir.join(".gitignore"));
        assert!(gi.is_ignored("foo.pyc", false));
        assert!(!gi.is_ignored("keep.pyc", false)); // negated
        assert!(gi.is_ignored("build", true));
        assert!(!gi.is_ignored("main.rs", false));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_gitignore_merge() {
        let dir = std::env::temp_dir().join("gref_test_gi_merge");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("parent.gitignore"), "*.log\n").unwrap();
        fs::write(dir.join("child.gitignore"), "!important.log\n").unwrap();

        let parent = GitIgnore::from_path(&dir.join("parent.gitignore"));
        let merged = parent.merge_file(&dir.join("child.gitignore"));

        assert!(merged.is_ignored("debug.log", false));
        assert!(!merged.is_ignored("important.log", false)); // child overrides

        let _ = fs::remove_dir_all(&dir);
    }
}
