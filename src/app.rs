use crate::model::{AppMode, AppState, Model};
use crate::replace;
use crate::term::{self, Key};
use crate::ui;

/// Run the TUI event loop.
pub fn run(model: &mut Model) -> Result<(), String> {
    term::enable_raw_mode();
    term::enter_alt_screen();

    // Install a panic hook that restores the terminal
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        term::disable_raw_mode();
        term::leave_alt_screen();
        default_hook(info);
    }));

    let result = event_loop(model);

    term::leave_alt_screen();
    term::disable_raw_mode();

    result
}

fn event_loop(model: &mut Model) -> Result<(), String> {
    loop {
        // Update terminal size
        let (cols, rows) = term::terminal_size();
        model.screen_width = cols as usize;
        model.screen_height = (rows as usize).saturating_sub(10).max(1);

        // Render (flicker-free repaint)
        let output = ui::render(model);
        term::paint(&output);

        // Read input
        if let Some(key) = term::read_key() {
            match model.state {
                AppState::Browse => handle_browse_key(model, key),
                AppState::Confirming => handle_confirming_key(model, key),
                AppState::Done | AppState::Replacing => {}
            }
        }

        // Check for quit conditions
        if model.state == AppState::Done {
            break;
        }

        if model.state == AppState::Replacing {
            // Perform replacement synchronously
            let result = replace::perform_replacements(
                &model.results,
                &model.selected,
                &model.pattern,
                &model.replacement_str,
            );
            match result {
                Ok(()) => model.state = AppState::Done,
                Err(e) => {
                    model.error = Some(e);
                    model.state = AppState::Done;
                }
            }

            // Render the final screen
            let output = ui::render(model);
            term::paint(&output);

            // Brief pause so user sees the result
            std::thread::sleep(std::time::Duration::from_millis(800));
            break;
        }
    }

    Ok(())
}

fn handle_browse_key(model: &mut Model, key: Key) {
    match key {
        Key::CtrlC | Key::Char('q') => {
            model.state = AppState::Done;
        }
        Key::Up | Key::Char('k') => {
            if model.cursor > 0 {
                model.cursor -= 1;
                if model.cursor < model.topline {
                    model.topline = model.cursor;
                }
            }
        }
        Key::Down | Key::Char('j') => {
            if model.cursor < model.results.len().saturating_sub(1) {
                model.cursor += 1;
                if model.cursor >= model.topline + model.screen_height {
                    model.topline = model.cursor - model.screen_height + 1;
                }
            }
        }
        Key::Left | Key::Char('h') => {
            if model.horizontal_offset > 0 {
                model.horizontal_offset = model.horizontal_offset.saturating_sub(10);
            }
        }
        Key::Right | Key::Char('l') => {
            let available_width = model.screen_width.saturating_sub(20).max(1);
            let end_line = (model.topline + model.screen_height).min(model.results.len());
            let mut max_offset = 0;
            for i in model.topline..end_line {
                let line_len = model.results[i].line_text.len();
                let offset = line_len.saturating_sub(available_width);
                if offset > max_offset {
                    max_offset = offset;
                }
            }
            model.horizontal_offset += 5;
            if model.horizontal_offset > max_offset {
                model.horizontal_offset = max_offset;
            }
        }
        Key::Home => {
            model.horizontal_offset = 0;
        }
        Key::End => {
            model.horizontal_offset = 1000;
        }
        Key::Space => {
            if model.mode != AppMode::SearchOnly {
                if model.selected.contains(&model.cursor) {
                    model.selected.remove(&model.cursor);
                } else {
                    model.selected.insert(model.cursor);
                }
            }
        }
        Key::Char('a') => {
            if model.mode != AppMode::SearchOnly {
                for i in 0..model.results.len() {
                    model.selected.insert(i);
                }
            }
        }
        Key::Char('n') => {
            if model.mode != AppMode::SearchOnly {
                model.selected.clear();
            }
        }
        Key::Enter => {
            if model.mode != AppMode::SearchOnly {
                if model.selected.is_empty() {
                    model.error = Some("no results".to_string());
                } else {
                    model.state = AppState::Confirming;
                }
            }
        }
        _ => {}
    }
}

fn handle_confirming_key(model: &mut Model, key: Key) {
    match key {
        Key::Enter => {
            model.state = AppState::Replacing;
        }
        Key::Escape => {
            model.state = AppState::Browse;
            model.error = None;
        }
        Key::CtrlC | Key::Char('q') => {
            model.state = AppState::Done;
        }
        _ => {}
    }
}
