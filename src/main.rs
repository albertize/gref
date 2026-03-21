use gref::app;
use gref::cli;
use gref::model;
use gref::search;

fn main() {
    let args = cli::parse();

    // Compile regex
    let pattern_str = if args.ignore_case {
        format!("(?i){}", args.pattern)
    } else {
        args.pattern.clone()
    };
    let pattern = match regex::Regex::new(&pattern_str) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error compiling regex pattern: {}", e);
            std::process::exit(1);
        }
    };

    // Determine mode
    let mode = if args.replacement.is_some() {
        model::AppMode::Default
    } else {
        model::AppMode::SearchOnly
    };

    // Perform search
    let results = match search::perform_search_adaptive(
        &args.root_path,
        &pattern,
        &args.exclude,
        !args.hidden,
        !args.no_ignore,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error during search: {}", e);
            std::process::exit(1);
        }
    };

    if results.is_empty() {
        println!("No results found for the pattern: {}", args.pattern);
        std::process::exit(0);
    }

    // Initialize model and run TUI
    let mut m = model::Model::new(
        results,
        args.pattern,
        args.replacement.unwrap_or_default(),
        pattern,
        mode,
    );

    if let Err(e) = app::run(&mut m) {
        eprintln!("Error running the program: {}", e);
        std::process::exit(1);
    }
}
