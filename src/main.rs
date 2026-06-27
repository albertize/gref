use gref::app;
use gref::cli;
use gref::integration;
use gref::model;
use gref::search;
use std::path::Path;

fn write_vim_error_if_needed(vim_result: Option<&str>, message: &str) {
    if let Some(path) = vim_result {
        let _ = integration::write_vim_error(Path::new(path), message);
    }
}

fn main() {
    let args = cli::parse();
    let vim_result = args.vim_result.clone();

    let pattern = match search::compile_search_pattern(&args.pattern, args.ignore_case, args.regex)
    {
        Ok(p) => p,
        Err(e) => {
            let message = format!("Error compiling search pattern: {}", e);
            write_vim_error_if_needed(vim_result.as_deref(), &message);
            eprintln!("{}", message);
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
            let message = format!("Error during search: {}", e);
            write_vim_error_if_needed(vim_result.as_deref(), &message);
            eprintln!("{}", message);
            std::process::exit(1);
        }
    };

    if results.is_empty() {
        if let Some(path) = vim_result.as_deref() {
            if let Err(e) = integration::write_vim_no_results(Path::new(path)) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
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
    m.editor_open_enabled = vim_result.is_none();

    if let Some(path) = vim_result.as_deref() {
        let _ = std::fs::remove_file(path);
    }

    if let Err(e) = app::run(&mut m) {
        let message = format!("Error running the program: {}", e);
        write_vim_error_if_needed(vim_result.as_deref(), &message);
        eprintln!("{}", message);
        std::process::exit(1);
    }

    if let Some(path) = vim_result.as_deref() {
        let result_path = Path::new(path);
        let write_result = match mode {
            model::AppMode::SearchOnly => {
                if let Some(idx) = m.selected_result {
                    if let Some(result) = m.results.get(idx) {
                        integration::write_vim_selected_result(result_path, result, &m.pattern)
                    } else {
                        integration::write_vim_cancelled(result_path)
                    }
                } else {
                    integration::write_vim_cancelled(result_path)
                }
            }
            model::AppMode::Default => {
                if let Some(error) = m.error.as_deref() {
                    integration::write_vim_error(result_path, error)
                } else if m.replacement_performed {
                    integration::write_vim_replaced(result_path)
                } else {
                    integration::write_vim_cancelled(result_path)
                }
            }
        };
        if let Err(e) = write_result {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
