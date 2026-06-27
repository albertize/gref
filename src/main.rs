use gref::app;
use gref::cli;
use gref::integration;
use gref::model;
use gref::search;
use std::path::Path;

fn main() {
    let args = cli::parse();
    let vim_result = args.vim_result.clone();

    let pattern = match search::compile_search_pattern(&args.pattern, args.ignore_case, args.regex)
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error compiling search pattern: {}", e);
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
        search::default_skip_hidden(&args.root_path, args.hidden),
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
        args.regex,
    );
    m.select_result_on_enter = vim_result.is_some() && mode == model::AppMode::SearchOnly;

    if let Some(path) = vim_result.as_deref() {
        let _ = std::fs::remove_file(path);
    }

    if let Err(e) = app::run(&mut m) {
        eprintln!("Error running the program: {}", e);
        std::process::exit(1);
    }

    if let (Some(path), Some(idx)) = (vim_result.as_deref(), m.selected_result) {
        if let Some(result) = m.results.get(idx) {
            if let Err(e) = integration::write_vim_result(Path::new(path), result) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }
}
