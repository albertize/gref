use crate::model::{AppMode, AppState, Model};
use crate::replace;
use crate::term::{self, Key};
use crate::ui;
use std::process::Command;

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

    if !model.terminal_released {
        term::leave_alt_screen();
        term::disable_raw_mode();
    }

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
                AppState::Browse => handle_browse_key(model, key)?,
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
            let result = replace::perform_replacements_with_options(
                &model.results,
                &model.selected,
                &model.pattern,
                &model.replacement_str,
                model.regex_mode,
            );
            match result {
                Ok(()) => {
                    model.replacement_performed = true;
                    model.state = AppState::Done;
                }
                Err(e) => {
                    model.error = Some(e);
                    model.state = AppState::Done;
                }
            }

            // Render the final screen
            let output = ui::render(model);
            term::paint(&output);
            break;
        }
    }

    Ok(())
}

fn open_current_result_in_editor(model: &mut Model) -> Result<(), String> {
    let Some(result) = model.results.get(model.cursor) else {
        return Ok(());
    };

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vim".to_string());

    term::leave_alt_screen();
    term::disable_raw_mode();

    let status = Command::new(&editor)
        .arg(format!("+{}", result.line_num))
        .arg(result.path())
        .status()
        .map_err(|e| format!("failed to open editor '{}': {}", editor, e));

    match status {
        Ok(status) if status.success() => {
            model.terminal_released = true;
            model.state = AppState::Done;
            Ok(())
        }
        Ok(status) => {
            term::enable_raw_mode();
            term::enter_alt_screen();
            model.error = Some(format!("editor '{}' exited with status {}", editor, status));
            Ok(())
        }
        Err(e) => {
            term::enable_raw_mode();
            term::enter_alt_screen();
            model.error = Some(e);
            Ok(())
        }
    }
}

fn handle_browse_key(model: &mut Model, key: Key) -> Result<(), String> {
    match key {
        Key::CtrlC | Key::Char('q') => {
            model.state = AppState::Done;
        }
        Key::Up | Key::Char('k') if model.cursor > 0 => {
            model.cursor -= 1;
        }
        Key::Down | Key::Char('j') if model.cursor < model.results.len().saturating_sub(1) => {
            model.cursor += 1;
        }
        Key::Left | Key::Char('h') if model.horizontal_offset > 0 => {
            model.horizontal_offset = model.horizontal_offset.saturating_sub(10);
        }
        Key::Right | Key::Char('l') => {
            let available_width = model.screen_width.saturating_sub(20).max(1);
            let max_line_len = model
                .results
                .iter()
                .map(|r| r.line_text.len())
                .max()
                .unwrap_or(0);
            let max_offset = max_line_len.saturating_sub(available_width);
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
        Key::Char('v') if model.editor_open_enabled => {
            open_current_result_in_editor(model)?;
        }
        Key::Space if model.mode != AppMode::SearchOnly => {
            if model.selected.contains(&model.cursor) {
                model.selected.remove(&model.cursor);
            } else {
                model.selected.insert(model.cursor);
            }
        }
        Key::Char('a') if model.mode != AppMode::SearchOnly => {
            for i in 0..model.results.len() {
                model.selected.insert(i);
            }
        }
        Key::Char('n') if model.mode != AppMode::SearchOnly => {
            model.selected.clear();
        }
        Key::Enter if model.mode != AppMode::SearchOnly => {
            if model.selected.is_empty() {
                model.error = Some("no results".to_string());
            } else {
                model.state = AppState::Confirming;
            }
        }
        Key::Enter if model.select_result_on_enter && !model.results.is_empty() => {
            model.selected_result = Some(model.cursor);
            model.state = AppState::Done;
        }
        _ => {}
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SearchResult;

    fn model(mode: AppMode) -> Model {
        Model::new(
            vec![SearchResult::from_display_path("a.rs", 7, "foo")],
            "foo".to_string(),
            String::new(),
            regex::Regex::new("foo").unwrap(),
            mode,
            false,
        )
    }

    #[test]
    fn enter_in_search_only_selects_result_when_enabled() {
        let mut model = model(AppMode::SearchOnly);
        model.select_result_on_enter = true;

        handle_browse_key(&mut model, Key::Enter).unwrap();

        assert_eq!(model.selected_result, Some(0));
        assert_eq!(model.state, AppState::Done);
    }

    #[test]
    fn enter_in_search_only_stays_noop_without_integration() {
        let mut model = model(AppMode::SearchOnly);

        handle_browse_key(&mut model, Key::Enter).unwrap();

        assert_eq!(model.selected_result, None);
        assert_eq!(model.state, AppState::Browse);
    }
}
