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
}

const HELP_TEXT: &str = r#"gref - search and replace tool

Usage:
  gref [options] <pattern> [replacement] [directory]

Options:
  -h, --help          Show this help message and exit
  -i, --ignore-case   Ignore case in pattern matching
  -e, --exclude       Exclude path, file or extension (comma separated, e.g. ".git,*.log,media/")
  --hidden            Include hidden files and directories (default: skip)
  --no-ignore         Don't respect .gitignore files

Arguments:
  <pattern>         Regex pattern to search for
  [replacement]     Replacement string (if omitted, only search)
  [directory]       Directory to search (default: current directory)

Examples:
  gref foo bar src      Replace 'foo' with 'bar' in src directory
  gref foo              Search for 'foo' only
  gref -i Foo           Search for 'Foo' (case-insensitive)
  gref --help           Show help message
  gref -e .git,*.log    Exclude .git folders and .log files
"#;

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
    let mut exclude_str = String::new();
    let mut positional: Vec<String> = Vec::new();

    let mut i = 0;
    while i < raw.len() {
        let arg = &raw[i];
        match arg.as_str() {
            "-h" | "--help" => show_help = true,
            "-i" | "--ignore-case" => ignore_case = true,
            "--hidden" => hidden = true,
            "--no-ignore" => no_ignore = true,
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
    let root_path = positional.get(2).cloned().unwrap_or_else(|| ".".to_string());

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
        assert!(cli.exclude.is_empty());
    }

    #[test]
    fn test_parse_from_search_only() {
        let args: Vec<String> = vec!["foo".into()];
        let cli = parse_from(&args);
        assert_eq!(cli.pattern, "foo");
        assert!(cli.replacement.is_none());
        assert_eq!(cli.root_path, ".");
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
}
