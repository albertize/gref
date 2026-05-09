use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_IGNORE_FILE_SIZE: u64 = 1024 * 1024;
const MAX_IGNORE_RULES: usize = 16 * 1024;
const MAX_IGNORE_PATTERN_LEN: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatchKind {
    Basename,
    RelativePath,
}

#[derive(Clone, Debug)]
struct IgnoreRule {
    regex: Regex,
    match_kind: MatchKind,
    is_negation: bool,
    dir_only: bool,
}

#[derive(Clone, Debug)]
struct IgnoreScope {
    base_dir: PathBuf,
    rules: Vec<IgnoreRule>,
}

#[derive(Clone, Debug)]
pub struct GitIgnore {
    scopes: Vec<IgnoreScope>,
}

fn ignore_error(path: &Path, message: &str) -> String {
    format!("{}: {}", path.display(), message)
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
/// anchored patterns (`/` prefix), basename matching, and path patterns relative
/// to the ignore file's directory.
fn parse_line(line: &str) -> Result<Option<IgnoreRule>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
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

    let anchored = s.starts_with('/');
    let s = s.strip_prefix('/').unwrap_or(s);

    if s.is_empty() {
        return Ok(None);
    }
    if s.len() > MAX_IGNORE_PATTERN_LEN {
        return Err(format!(
            "ignore pattern exceeds {} bytes",
            MAX_IGNORE_PATTERN_LEN
        ));
    }

    let match_kind = if anchored || s.contains('/') {
        MatchKind::RelativePath
    } else {
        MatchKind::Basename
    };

    let regex_str = glob_to_regex(s);
    let regex =
        Regex::new(&regex_str).map_err(|e| format!("invalid ignore pattern '{}': {}", line, e))?;

    Ok(Some(IgnoreRule {
        regex,
        match_kind,
        is_negation,
        dir_only,
    }))
}

fn normalize_path_for_matching(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl IgnoreRule {
    fn matches(&self, path: &Path, base_dir: &Path, is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }

        let scoped_path = if base_dir.as_os_str().is_empty() {
            Some(path)
        } else {
            path.strip_prefix(base_dir).ok()
        };
        let scoped_path = match scoped_path {
            Some(p) => p,
            None => return false,
        };

        match self.match_kind {
            MatchKind::Basename => {
                let name = match scoped_path.file_name() {
                    Some(name) => name.to_string_lossy(),
                    None => return false,
                };
                self.regex.is_match(name.as_ref())
            }
            MatchKind::RelativePath => {
                let relative = normalize_path_for_matching(scoped_path);
                self.regex.is_match(&relative)
            }
        }
    }
}

impl GitIgnore {
    pub fn empty() -> Self {
        GitIgnore { scopes: Vec::new() }
    }

    pub fn from_path(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Ok(Self::empty());
        }

        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                return Err(ignore_error(
                    path,
                    &format!("failed to read ignore file metadata: {}", e),
                ));
            }
        };
        if metadata.len() > MAX_IGNORE_FILE_SIZE {
            return Err(ignore_error(
                path,
                &format!("ignore file exceeds {} bytes", MAX_IGNORE_FILE_SIZE),
            ));
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                return Err(ignore_error(
                    path,
                    &format!("failed to read ignore file: {}", e),
                ));
            }
        };

        let mut rules = Vec::new();
        for (line_idx, line) in content.lines().enumerate() {
            if line_idx >= MAX_IGNORE_RULES {
                return Err(ignore_error(
                    path,
                    &format!("ignore file exceeds {} rules", MAX_IGNORE_RULES),
                ));
            }
            if let Some(rule) = parse_line(line)
                .map_err(|e| ignore_error(path, &format!("line {}: {}", line_idx + 1, e)))?
            {
                rules.push(rule);
            }
        }
        if rules.is_empty() {
            return Ok(Self::empty());
        }

        let base_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(GitIgnore {
            scopes: vec![IgnoreScope { base_dir, rules }],
        })
    }

    /// Merge with rules from a child ignore file.
    /// Parent rules come first, child rules last (last match wins).
    pub fn merge_file(&self, path: &Path) -> Result<GitIgnore, String> {
        let child = GitIgnore::from_path(path)?;
        if child.scopes.is_empty() {
            return Ok(self.clone());
        }
        let mut scopes = self.scopes.clone();
        scopes.extend(child.scopes);
        Ok(GitIgnore { scopes })
    }

    /// Merge all ignore files (.gitignore, .ignore, .grefignore) from a directory.
    /// Priority (last match wins): .gitignore < .ignore < .grefignore.
    pub fn merge_dir(&self, dir: &Path) -> Result<GitIgnore, String> {
        const IGNORE_FILES: &[&str] = &[".gitignore", ".ignore", ".grefignore"];
        let mut result = self.clone();
        for name in IGNORE_FILES {
            let path = dir.join(name);
            if path.is_file() {
                result = result.merge_file(&path)?;
            }
        }
        Ok(result)
    }

    /// Check whether a name should be ignored. Last matching rule wins.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let mut ignored = false;
        for scope in &self.scopes {
            for rule in &scope.rules {
                if rule.matches(path, &scope.base_dir, is_dir) {
                    ignored = !rule.is_negation;
                }
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
pub fn load_ancestor_gitignores(root: &Path) -> Result<GitIgnore, String> {
    let mut ancestor_dirs = Vec::new();
    let mut found_repo_root = false;
    let mut current = match root.parent() {
        Some(p) => p.to_path_buf(),
        None => return Ok(GitIgnore::empty()),
    };

    loop {
        ancestor_dirs.push(current.clone());
        if current.join(".git").exists() {
            found_repo_root = true;
            break;
        }
        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => break,
        }
    }

    if !found_repo_root {
        return Ok(GitIgnore::empty());
    }

    // Apply from repo root (last) down to closest ancestor (first)
    ancestor_dirs.reverse();
    let mut result = GitIgnore::empty();
    for dir in &ancestor_dirs {
        for name in IGNORE_FILES {
            let path = dir.join(name);
            if path.is_file() {
                result = result.merge_file(&path)?;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn gitignore_from_rules(rules: Vec<IgnoreRule>) -> GitIgnore {
        GitIgnore {
            scopes: vec![IgnoreScope {
                base_dir: PathBuf::new(),
                rules,
            }],
        }
    }

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
        assert!(parse_line("# comment").unwrap().is_none());
        assert!(parse_line("  ").unwrap().is_none());
        assert!(parse_line("").unwrap().is_none());
    }

    #[test]
    fn test_parse_line_simple() {
        let rule = parse_line("*.pyc").unwrap().unwrap();
        assert!(!rule.is_negation);
        assert!(!rule.dir_only);
        assert_eq!(rule.match_kind, MatchKind::Basename);
        assert!(rule.regex.is_match("foo.pyc"));
        assert!(!rule.regex.is_match("foo.py"));
    }

    #[test]
    fn test_parse_line_dir_only() {
        let rule = parse_line("build/").unwrap().unwrap();
        assert!(rule.dir_only);
        assert!(rule.regex.is_match("build"));
    }

    #[test]
    fn test_parse_line_negation() {
        let rule = parse_line("!important.log").unwrap().unwrap();
        assert!(rule.is_negation);
        assert!(rule.regex.is_match("important.log"));
    }

    #[test]
    fn test_parse_line_leading_slash() {
        let rule = parse_line("/target").unwrap().unwrap();
        assert_eq!(rule.match_kind, MatchKind::RelativePath);
        assert!(rule.regex.is_match("target"));
        assert!(!rule.regex.is_match("nested/target"));
    }

    #[test]
    fn test_parse_line_path_pattern_supported() {
        let file_rule = parse_line("docs/build").unwrap().unwrap();
        assert_eq!(file_rule.match_kind, MatchKind::RelativePath);
        assert!(file_rule.regex.is_match("docs/build"));
        assert!(!file_rule.regex.is_match("other/docs/build"));

        let dir_rule = parse_line("src/generated/").unwrap().unwrap();
        assert_eq!(dir_rule.match_kind, MatchKind::RelativePath);
        assert!(dir_rule.dir_only);
        assert!(dir_rule.regex.is_match("src/generated"));
    }

    #[test]
    fn test_parse_line_rejects_oversized_pattern() {
        let pattern = format!("{}{}", "a".repeat(MAX_IGNORE_PATTERN_LEN), "b");
        let err = parse_line(&pattern).unwrap_err();
        assert!(err.contains("ignore pattern exceeds"));
    }

    #[test]
    fn test_gitignore_is_ignored() {
        let gi = gitignore_from_rules(vec![
            parse_line("*.pyc").unwrap().unwrap(),
            parse_line("build/").unwrap().unwrap(),
            parse_line("*.log").unwrap().unwrap(),
            parse_line("!important.log").unwrap().unwrap(),
        ]);
        assert!(gi.is_ignored(Path::new("foo.pyc"), false));
        assert!(gi.is_ignored(Path::new("build"), true));
        assert!(!gi.is_ignored(Path::new("build"), false)); // dir_only
        assert!(gi.is_ignored(Path::new("debug.log"), false));
        assert!(!gi.is_ignored(Path::new("important.log"), false)); // negated
        assert!(!gi.is_ignored(Path::new("main.rs"), false));
    }

    #[test]
    fn test_gitignore_empty() {
        let gi = GitIgnore::empty();
        assert!(!gi.is_ignored(Path::new("anything"), false));
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

        let gi = GitIgnore::from_path(&dir.join(".gitignore")).unwrap();
        assert!(gi.is_ignored(&dir.join("foo.pyc"), false));
        assert!(!gi.is_ignored(&dir.join("keep.pyc"), false)); // negated
        assert!(gi.is_ignored(&dir.join("build"), true));
        assert!(!gi.is_ignored(&dir.join("main.rs"), false));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_gitignore_path_patterns_match_relative_to_ignore_file() {
        let dir = std::env::temp_dir().join("gref_test_gitignore_paths");
        let nested = dir.join("nested/generated");
        let vendor_cache = dir.join("vendor/cache");
        let _ = fs::create_dir_all(&nested);
        let _ = fs::create_dir_all(&vendor_cache);
        fs::write(
            dir.join(".gitignore"),
            "nested/generated/\nvendor/cache/*.txt\n",
        )
        .unwrap();

        let gi = GitIgnore::from_path(&dir.join(".gitignore")).unwrap();
        assert!(gi.is_ignored(&nested, true));
        assert!(gi.is_ignored(&vendor_cache.join("secret.txt"), false));
        assert!(!gi.is_ignored(&dir.join("nested/secret.txt"), false));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_gitignore_merge() {
        let dir = std::env::temp_dir().join("gref_test_gi_merge");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("parent.gitignore"), "*.log\n").unwrap();
        fs::write(dir.join("child.gitignore"), "!important.log\n").unwrap();

        let parent = GitIgnore::from_path(&dir.join("parent.gitignore")).unwrap();
        let merged = parent.merge_file(&dir.join("child.gitignore")).unwrap();

        assert!(merged.is_ignored(&dir.join("debug.log"), false));
        assert!(!merged.is_ignored(&dir.join("important.log"), false)); // child overrides

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_gitignore_from_file_rejects_oversized_ignore() {
        let dir = std::env::temp_dir().join("gref_test_gi_oversized");
        let _ = fs::create_dir_all(&dir);
        fs::write(
            dir.join(".gitignore"),
            vec![b'a'; MAX_IGNORE_FILE_SIZE as usize + 1],
        )
        .unwrap();

        let err = GitIgnore::from_path(&dir.join(".gitignore")).unwrap_err();
        assert!(err.contains("ignore file exceeds"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_gitignore_from_file_rejects_too_many_rules() {
        let dir = std::env::temp_dir().join("gref_test_gi_too_many_rules");
        let _ = fs::create_dir_all(&dir);
        let content = (0..=MAX_IGNORE_RULES)
            .map(|idx| format!("rule{}\n", idx))
            .collect::<String>();
        fs::write(dir.join(".gitignore"), content).unwrap();

        let err = GitIgnore::from_path(&dir.join(".gitignore")).unwrap_err();
        assert!(err.contains("ignore file exceeds"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_ancestor_gitignores_requires_repo_boundary() {
        let parent = std::env::temp_dir().join("gref_test_gi_no_repo_boundary");
        let project = parent.join("project");
        let sub = project.join("sub");
        let _ = fs::remove_dir_all(&parent);
        let _ = fs::create_dir_all(project.join(".git"));
        let _ = fs::create_dir_all(&sub);
        fs::write(parent.join(".gitignore"), "*.txt\n").unwrap();

        let gi = load_ancestor_gitignores(&sub).unwrap();
        assert!(!gi.is_ignored(&sub.join("target.txt"), false));

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn test_load_ancestor_gitignores_respects_repo_boundary() {
        let repo = std::env::temp_dir().join("gref_test_gi_repo_boundary");
        let sub = repo.join("sub");
        let _ = fs::create_dir_all(repo.join(".git"));
        let _ = fs::create_dir_all(&sub);
        fs::write(repo.join(".gitignore"), "*.log\n").unwrap();

        let gi = load_ancestor_gitignores(&sub).unwrap();
        assert!(gi.is_ignored(&sub.join("debug.log"), false));
        assert!(!gi.is_ignored(&sub.join("main.txt"), false));

        let _ = fs::remove_dir_all(&repo);
    }
}
