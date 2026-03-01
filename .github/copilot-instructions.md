# Copilot Instructions — gref

## Project Overview

A single-binary terminal TUI for regex search-and-replace across directory trees. No TUI framework — raw ANSI escapes + platform FFI for terminal control.

## Architecture

```
main.rs          → CLI parse, regex compile, search, model init, app::run()
lib.rs           → pub mod re-exports (enables integration tests)
cli.rs           → Manual arg parsing (no clap). CliArgs struct.
model.rs         → SearchResult, AppState (Browse→Confirming→Replacing→Done), AppMode, Model
search.rs        → File walking, parallel regex search (std::thread + mpsc channels)
replace.rs       → Atomic file replacement via temp file + rename
term.rs          → Raw mode FFI (Windows kernel32 / Unix termios), ANSI escapes, Key enum, paint()
ui.rs            → Screen rendering into a String (render() → term::paint())
app.rs           → Event loop: read_key → dispatch → render cycle
exclude.rs       → Path exclusion (dir/, *.ext, exact filename patterns)
filedetect.rs    → Text vs binary detection (extension lookup + 512-byte content probe)
```

**Data flow:** `cli::parse()` → `search::perform_search_adaptive()` → `Model::new()` → `app::run()` (loop: `ui::render()` → `term::paint()` → `term::read_key()` → state update) → `replace::perform_replacements()`.

## Key Design Decisions

- **Zero external TUI deps**: Terminal is managed via direct platform FFI (`SetConsoleMode` on Windows, `tcsetattr` on Unix). See `term.rs` platform modules.
- **Only dependency is `regex = "1"`** — no clap, crossterm, walkdir, or rayon.
- **Flicker-free rendering**: `term::paint()` uses cursor-home + per-line clear-to-EOL + clear-to-EOS in a single locked stdout write. Never use `CLEAR_SCREEN` (`\x1b[2J`).
- **UTF-8 safe slicing**: Horizontal offset uses `char_indices().nth()` — never byte-index into display strings (see `ui.rs:85`).
- **Atomic replacement**: `replace_in_file()` writes to a temp file (`.gref_tmp_*`) then renames over the original.
- **Parallel search**: Thread pool with `Arc<Mutex<Receiver<PathBuf>>>` work-stealing pattern in `search::perform_search_adaptive()`.

## Build & Test

```powershell
cargo build                    # dev build
cargo build --release          # release (strip=true, lto=true, opt-level="z")
cargo test                     # 25 unit + 87 stress/edge-case tests
cargo clippy                   # must pass with 0 warnings
.\make.ps1                     # cross-compile to dist/ (linux/darwin/windows amd64)
```

## Test Structure

- **Unit tests**: Inline `#[cfg(test)] mod tests` in `cli.rs`, `exclude.rs`, `filedetect.rs`, `search.rs`, `replace.rs`
- **Integration/stress tests**: `tests/stress_tests.rs` — uses `gref::` (lib) imports, covers all modules including UI rendering and key-handling simulation
- Tests use `std::env::temp_dir()` for file I/O with `gref_stress_*` / `gref_test_*` prefixed filenames

## Conventions

- All public API in modules; `lib.rs` re-exports, `main.rs` consumes via `use gref::*`
- Error handling: `Result<(), String>` for fallible ops; `eprintln!` + `process::exit(1)` only in `main()`
- No `unwrap()` in library code paths that touch user files — propagate errors
- Style helpers in `term.rs` (`style_red`, `style_green`, etc.) wrap text with ANSI codes + RESET
- `Model` is the single source of truth — all state mutation happens in `app.rs` key handlers; `ui.rs` is pure rendering
- When adding new file extensions, maintain sorted order in `filedetect.rs` arrays (binary search)
