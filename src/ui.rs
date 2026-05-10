use crate::model::{AppMode, AppState, Model};
use crate::term;
use std::fmt::Write as _;
use std::sync::Arc;

/// Count the number of decimal digits in a number.
fn digit_count(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    let mut v = n;
    while v > 0 {
        count += 1;
        v /= 10;
    }
    count
}

/// A visible line in the TUI — either a file header or a result row.
struct VisibleLine {
    is_header: bool,
    file: Arc<str>,
    idx: Option<usize>, // index into model.results (None for headers)
}

fn sanitize_terminal_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for ch in text.chars() {
        match ch {
            '\t' => out.push_str("    "),
            c if c.is_control() => {
                let mut buf = [0u8; 4];
                for byte in c.encode_utf8(&mut buf).as_bytes() {
                    let _ = write!(out, "\\x{:02X}", byte);
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

/// Build the list of visible lines (file headers interleaved with results).
fn build_visible_lines(model: &Model) -> Vec<VisibleLine> {
    let mut lines = Vec::with_capacity(model.results.len() * 2);
    for (i, res) in model.results.iter().enumerate() {
        if i == 0 || res.path() != model.results[i - 1].path() {
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

fn header_topline_for_cursor(visible_lines: &[VisibleLine], cursor_line: usize) -> usize {
    if cursor_line > 0 && visible_lines[cursor_line - 1].is_header {
        cursor_line - 1
    } else {
        cursor_line
    }
}

/// Render the complete screen content into a single String.
pub fn render(model: &mut Model) -> String {
    let mut s = String::with_capacity(4096);

    s.push_str(&render_header(model));

    if model.state == AppState::Browse {
        let visible_lines = build_visible_lines(model);

        // Find cursor line index in the visible_lines list
        let cursor_line = visible_lines
            .iter()
            .position(|v| !v.is_header && v.idx == Some(model.cursor))
            .unwrap_or(0);

        // Adjust topline so cursor is visible
        if model.screen_height > 1 && cursor_line <= model.topline {
            model.topline = header_topline_for_cursor(&visible_lines, cursor_line);
        } else if cursor_line < model.topline {
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
                let prefix_len = 5; // "DIR: "
                let max_path = (model.screen_width - 1).saturating_sub(prefix_len);
                let safe_file = sanitize_terminal_text(&v.file);
                let truncated_file: String = safe_file.chars().take(max_path).collect();
                s.push_str(&format!("DIR: {}\n", truncated_file));
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
                let line = sanitize_terminal_text(&res.line_text);
                let byte_offset = line
                    .char_indices()
                    .nth(model.horizontal_offset)
                    .map(|(i, _)| i)
                    .unwrap_or(line.len());
                let visible_line = &line[byte_offset..];

                // Truncate to available width so the terminal never wraps.
                // Use screen_width - 1 to avoid writing the last column (auto-wrap).
                let prefix_width = 2 + 3 + 1 + digit_count(res.line_num) + 2;
                let text_width = (model.screen_width - 1).saturating_sub(prefix_width);
                let trunc_end = visible_line
                    .char_indices()
                    .nth(text_width)
                    .map(|(i, _)| i)
                    .unwrap_or(visible_line.len());
                let truncated = &visible_line[..trunc_end];

                // Build the styled line text with match highlighting
                s.push_str(&format!("{}{} {}: ", cursor_str, checked_str, res.line_num));

                let mut last_index = 0;
                for (start, matched) in truncated.match_indices(&model.pattern_str) {
                    // Text before the match
                    s.push_str(&truncated[last_index..start]);

                    if is_selected {
                        s.push_str(&term::style_cyan_bold(&sanitize_terminal_text(
                            &model.replacement_str,
                        )));
                    } else {
                        s.push_str(&term::style_red(matched));
                    }
                    last_index = start + matched.len();
                }
                // Remaining text after last match
                s.push_str(&truncated[last_index..]);
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
        s.push_str(&term::style_red_bold(&format!(
            "Error: {}\n",
            sanitize_terminal_text(err)
        )));
        s.push_str("\nPress 'q' to exit.\n");
        return s;
    }

    match model.state {
        AppState::Browse => {
            s.push_str("--- Search results (Pattern: ");
            s.push_str(&term::style_red(&sanitize_terminal_text(
                &model.pattern_str,
            )));
            s.push_str(") ---\n");
            match model.mode {
                AppMode::SearchOnly => {
                    s.push_str("Search Only Mode\n");
                }
                AppMode::Default => {
                    s.push_str("Replacing with: ");
                    s.push_str(&term::style_green(&sanitize_terminal_text(
                        &model.replacement_str,
                    )));
                    s.push('\n');
                }
            }
            s.push('\n');
        }
        AppState::Confirming => {
            s.push_str(&format!("Replacing {}?\n", model.selected.len()));
            s.push_str(&format!(
                "Pattern: {} -> Replace: {}\n\n",
                term::style_red(&sanitize_terminal_text(&model.pattern_str)),
                term::style_green(&sanitize_terminal_text(&model.replacement_str))
            ));
        }
        AppState::Replacing => {
            s.push_str("Replacing... wait.\n");
        }
        AppState::Done => {}
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
                "\nup/down /j/k: move | left/right /h/l: scroll horizontally | Home/End: scroll to start/end of line | v: open in $EDITOR",
            ));
            match model.mode {
                AppMode::SearchOnly if model.select_result_on_enter => {
                    s.push_str(&term::style_grey("\nEnter: open | q/Ctrl+C: exit"));
                }
                AppMode::SearchOnly => {
                    s.push_str(&term::style_grey("\nq/Ctrl+C: exit"));
                }
                AppMode::Default => {
                    s.push_str(&term::style_grey(
                        "\nSpace: select/deselect | a: select all | n: deselect all",
                    ));
                }
            }
        }
        AppState::Confirming => {
            s.push_str(&term::style_grey("Enter: confirm | Esc: exit"));
        }
        _ => {}
    }

    s
}
