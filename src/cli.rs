/// Parsed command-line arguments.
#[allow(dead_code)]
pub struct CliArgs {
    pub pattern: String,
    pub replacement: Option<String>,
    pub root_path: String,
    pub ignore_case: bool,
    pub exclude: Vec<String>,
    pub show_help: bool,
    pub hidden: bool,
    pub no_ignore: bool,
    pub vim_result: Option<String>,
    pub root_override: Option<String>,
    pub regex: bool,
}

const HELP_TEXT: &str = r#"gref - search and replace tool

Usage:
  gref [options] <pattern> [replacement] [directory]

Options:
  -h, --help          Show this help message and exit
  -v, --version       Show version information and exit
  -i, --ignore-case   Ignore case in pattern matching
  -r, --regex         Treat <pattern> as a regular expression (default: literal text)
  -e, --exclude       Exclude path, file or extension (comma separated, e.g. ".git,*.log,media/")
  --hidden            Include hidden files and directories (default: skip outside Git repo roots)
  --no-ignore         Don't respect .gitignore files
  --vim-result FILE   Write selected search result for Vim integration
  --root PATH         Search this file or directory

Arguments:
  <pattern>         Literal text to search for, unless --regex is used
  [replacement]     Replacement string (if omitted, only search)
  [directory]       Directory to search (default: current directory)

Examples:
  gref foo bar src      Replace 'foo' with 'bar' in src directory
  gref foo              Search for 'foo' only
  gref -r 'foo.*bar'    Search with a regular expression
  gref -i Foo           Search for 'Foo' (case-insensitive)
  gref --version        Show version information
  gref --help           Show help message
  gref -e .git,*.log    Exclude .git folders and .log files
"#;

/// Return the package version display string.
pub fn version_text() -> String {
    format!("gref {}", env!("CARGO_PKG_VERSION"))
}

/// Parse command-line arguments from `std::env::args()`.
pub fn parse() -> CliArgs {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    parse_from(&raw)
}

/// Parse from a given slice of arguments (useful for testing).
pub fn parse_from(raw: &[String]) -> CliArgs {
    let mut ignore_case = false;
    let mut show_help = false;
    let mut hidden = false;
    let mut no_ignore = false;
    let mut vim_result = None;
    let mut root_override = None;
    let mut regex = false;
    let mut exclude_str = String::new();
    let mut positional: Vec<String> = Vec::new();

    let mut i = 0;
    while i < raw.len() {
        let arg = &raw[i];
        match arg.as_str() {
            "-h" | "--help" => show_help = true,
            "-v" | "--version" => {
                println!("{}", version_text());
                std::process::exit(0);
            }
            "-i" | "--ignore-case" => ignore_case = true,
            "-r" | "--regex" => regex = true,
            "--hidden" => hidden = true,
            "--no-ignore" => no_ignore = true,
            "--vim-result" => {
                i += 1;
                if i < raw.len() {
                    vim_result = Some(raw[i].clone());
                } else {
                    eprintln!("Error: --vim-result requires a value");
                    std::process::exit(1);
                }
            }
            "--root" => {
                i += 1;
                if i < raw.len() {
                    root_override = Some(raw[i].clone());
                } else {
                    eprintln!("Error: --root requires a value");
                    std::process::exit(1);
                }
            }
            "-e" | "--exclude" => {
                i += 1;
                if i < raw.len() {
                    exclude_str = raw[i].clone();
                } else {
                    eprintln!("Error: -e/--exclude requires a value");
                    std::process::exit(1);
                }
            }
            other if other.starts_with('-') => {
                eprintln!("Error: unknown option '{}'", other);
                std::process::exit(1);
            }
            _ => positional.push(arg.clone()),
        }
        i += 1;
    }

    if show_help {
        print!("{}", HELP_TEXT);
        std::process::exit(0);
    }

    if positional.is_empty() {
        println!("Usage: gref [options] <pattern> [replacement] [directory]");
        println!("Try 'gref --help' for more information.");
        std::process::exit(0);
    }

    let pattern = positional[0].clone();
    let replacement = positional.get(1).cloned();
    let root_path = root_override
        .clone()
        .or_else(|| positional.get(2).cloned())
        .unwrap_or_else(|| ".".to_string());

    let exclude = if exclude_str.is_empty() {
        Vec::new()
    } else {
        parse_exclude_list(&exclude_str)
    };

    CliArgs {
        pattern,
        replacement,
        root_path,
        ignore_case,
        exclude,
        show_help,
        hidden,
        no_ignore,
        vim_result,
        root_override,
        regex,
    }
}

/// Split a comma-separated exclude string into individual patterns.
pub fn parse_exclude_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exclude_list() {
        let result = parse_exclude_list("foo, bar ,baz/");
        assert_eq!(result, vec!["foo", "bar", "baz/"]);
    }

    #[test]
    fn test_parse_exclude_list_empty() {
        let result = parse_exclude_list("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_from_basic() {
        let args: Vec<String> = vec!["foo".into(), "bar".into(), "src".into()];
        let cli = parse_from(&args);
        assert_eq!(cli.pattern, "foo");
        assert_eq!(cli.replacement, Some("bar".into()));
        assert_eq!(cli.root_path, "src");
        assert!(!cli.ignore_case);
        assert!(!cli.regex);
        assert!(cli.exclude.is_empty());
    }

    #[test]
    fn test_parse_from_search_only() {
        let args: Vec<String> = vec!["foo".into()];
        let cli = parse_from(&args);
        assert_eq!(cli.pattern, "foo");
        assert!(cli.replacement.is_none());
        assert_eq!(cli.root_path, ".");
        assert!(!cli.regex);
    }

    #[test]
    fn test_parse_from_with_flags() {
        let args: Vec<String> = vec![
            "-i".into(),
            "-e".into(),
            ".git,*.log".into(),
            "foo".into(),
            "bar".into(),
        ];
        let cli = parse_from(&args);
        assert!(cli.ignore_case);
        assert_eq!(cli.exclude, vec![".git", "*.log"]);
        assert_eq!(cli.pattern, "foo");
        assert_eq!(cli.replacement, Some("bar".into()));
    }

    #[test]
    fn test_parse_from_with_vim_result() {
        let args: Vec<String> = vec![
            "--vim-result".into(),
            "/tmp/gref-result".into(),
            "foo".into(),
        ];
        let cli = parse_from(&args);
        assert_eq!(cli.pattern, "foo");
        assert!(cli.replacement.is_none());
        assert_eq!(cli.vim_result, Some("/tmp/gref-result".into()));
    }

    #[test]
    fn test_parse_from_with_root_override() {
        let args: Vec<String> = vec!["--root".into(), "src/main.rs".into(), "foo".into()];
        let cli = parse_from(&args);
        assert_eq!(cli.pattern, "foo");
        assert!(cli.replacement.is_none());
        assert_eq!(cli.root_path, "src/main.rs");
        assert_eq!(cli.root_override, Some("src/main.rs".into()));
    }

    #[test]
    fn test_parse_from_regex_flag() {
        let args: Vec<String> = vec!["--regex".into(), "foo.*bar".into()];
        let cli = parse_from(&args);
        assert!(cli.regex);
        assert_eq!(cli.pattern, "foo.*bar");
    }

    #[test]
    fn test_parse_from_short_regex_flag() {
        let args: Vec<String> = vec!["-r".into(), "foo.*bar".into()];
        let cli = parse_from(&args);
        assert!(cli.regex);
        assert_eq!(cli.pattern, "foo.*bar");
    }

    #[test]
    fn test_version_text_uses_package_version() {
        assert_eq!(
            version_text(),
            format!("gref {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
