use crate::model::{AppMode, AppState, Model};
use crate::term;

/// A visible line in the TUI — either a file header or a result row.
struct VisibleLine {
    is_header: bool,
    file: String,
    idx: Option<usize>, // index into model.results (None for headers)
}

/// Build the list of visible lines (file headers interleaved with results).
fn build_visible_lines(model: &Model) -> Vec<VisibleLine> {
    let mut lines = Vec::with_capacity(model.results.len() * 2);
    for (i, res) in model.results.iter().enumerate() {
        if i == 0 || res.file_path != model.results[i - 1].file_path {
            lines.push(VisibleLine {
                is_header: true,
                file: res.file_path.clone(),
                idx: None,
            });
        }
        lines.push(VisibleLine {
            is_header: false,
            file: res.file_path.clone(),
            idx: Some(i),
        });
    }
    lines
}

/// Render the complete screen content into a single String.
pub fn render(model: &mut Model) -> String {
    let mut s = String::with_capacity(4096);

    s.push_str(&render_header(model));

    if model.state == AppState::Browse {
        let display_re = regex::Regex::new(&regex::escape(&model.pattern_str)).unwrap();
        let visible_lines = build_visible_lines(model);

        // Find cursor line index in the visible_lines list
        let cursor_line = visible_lines
            .iter()
            .position(|v| !v.is_header && v.idx == Some(model.cursor))
            .unwrap_or(0);

        // Adjust topline so cursor is visible
        if cursor_line < model.topline {
            model.topline = cursor_line;
        }
        if cursor_line >= model.topline + model.screen_height {
            model.topline = cursor_line - model.screen_height + 1;
        }

        let end = visible_lines.len().min(model.topline + model.screen_height);

        for (lines_shown, v) in visible_lines[model.topline..end].iter().enumerate() {
            if lines_shown >= model.screen_height {
                break;
            }
            if v.is_header {
                s.push_str(&format!("DIR: {}\n", v.file));
            } else if let Some(idx) = v.idx {
                let res = &model.results[idx];
                let is_cursor = model.cursor == idx;
                let is_selected = model.selected.contains(&idx);

                // Cursor indicator
                let cursor_str = if is_cursor {
                    term::style_bold("> ")
                } else {
                    "  ".to_string()
                };

                // Checkbox
                let checked_str = if is_selected {
                    term::style_cyan_bold("[x]")
                } else {
                    "[ ]".to_string()
                };

                // Apply horizontal offset (char-based to avoid splitting multi-byte codepoints)
                let line = &res.line_text;
                let byte_offset = line
                    .char_indices()
                    .nth(model.horizontal_offset)
                    .map(|(i, _)| i)
                    .unwrap_or(line.len());
                let visible_line = &line[byte_offset..];

                // Build the styled line text with match highlighting
                s.push_str(&format!("{}{} {}: ", cursor_str, checked_str, res.line_num));

                let mut last_index = 0;
                for mat in display_re.find_iter(visible_line) {
                    // Text before the match
                    s.push_str(&visible_line[last_index..mat.start()]);

                    if is_selected {
                        s.push_str(&term::style_cyan_bold(&model.replacement_str));
                    } else {
                        s.push_str(&term::style_red(mat.as_str()));
                    }
                    last_index = mat.end();
                }
                // Remaining text after last match
                s.push_str(&visible_line[last_index..]);
                s.push('\n');
            }
        }
    }

    s.push_str(&render_footer(model));
    s
}

fn render_header(model: &Model) -> String {
    let mut s = String::new();

    if let Some(ref err) = model.error {
        s.push_str(&term::style_red_bold(&format!("Error: {}\n", err)));
        s.push_str("\nPress 'q' to exit.\n");
        return s;
    }

    match model.state {
        AppState::Browse => {
            s.push_str("--- Search results (Pattern: ");
            s.push_str(&term::style_red(&model.pattern_str));
            s.push_str(") ---\n");
            match model.mode {
                AppMode::SearchOnly => {
                    s.push_str("Search Only Mode\n");
                }
                AppMode::Default => {
                    s.push_str("Replacing with: ");
                    s.push_str(&term::style_green(&model.replacement_str));
                    s.push('\n');
                }
            }
            s.push('\n');
        }
        AppState::Confirming => {
            s.push_str(&format!("Replacing {}?\n", model.selected.len()));
            s.push_str(&format!(
                "Pattern: {} -> Replace: {}\n\n",
                term::style_red(&model.pattern_str),
                term::style_green(&model.replacement_str)
            ));
        }
        AppState::Replacing => {
            s.push_str("Replacing... wait.\n");
        }
        AppState::Done => {
            //s.push_str("Success.\n");
        }
    }

    s
}

fn render_footer(model: &Model) -> String {
    let mut s = String::new();

    match model.state {
        AppState::Browse => {
            s.push_str(&term::style_grey(&format!(
                "\nLine {}/{}",
                model.cursor + 1,
                model.results.len()
            )));
            s.push_str(&term::style_grey(
                "\nup/down /j/k: move | left/right /h/l: scroll horizontally | Home/End: scroll to start/end of line \nSpace: select/deselect | a: select all | n: deselect all",
            ));
        }
        AppState::Confirming => {
            s.push_str(&term::style_grey("Enter: confirm | Esc: exit"));
        }
        _ => {}
    }

    s
}
