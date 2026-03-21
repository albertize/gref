# Copilot Instructions — gref

## Project Overview

A single-binary terminal TUI for regex search-and-replace across directory trees. No TUI framework — raw ANSI escapes + platform FFI for terminal control.

## Architecture

```
main.rs          → CLI parse, regex compile, search, model init, app::run()
lib.rs           → pub mod re-exports (enables integration tests)
cli.rs           → Manual arg parsing (no clap). CliArgs struct. Flags: -i, -e, --hidden, --no-ignore.
model.rs         → SearchResult { file_path, line_num, line_text }, AppState, AppMode, Model
search.rs        → Pipelined file walking + parallel bytes::Regex search, literal prefilter, hidden/gitignore skipping
replace.rs       → Atomic file replacement via temp file + rename
term.rs          → Raw mode FFI (Windows kernel32 / Unix termios), ANSI escapes, Key enum, paint()
ui.rs            → Screen rendering into a String (render() → term::paint())
app.rs           → Event loop: read_key → dispatch → render cycle
exclude.rs       → Path exclusion (dir/, *.ext, exact filename patterns). Uses Cow for path normalization.
filedetect.rs    → Text vs binary detection (64 text extensions + 512-byte content probe + SIMD null-byte scan)
gitignore.rs     → .gitignore/.ignore/.grefignore parsing (glob→regex), hierarchical rule merging, ancestor discovery
```

**Data flow:** `cli::parse()` → `search::perform_search_adaptive()` → `Model::new()` → `app::run()` (loop: `ui::render()` → `term::paint()` → `term::read_key()` → state update) → `replace::perform_replacements()`.

## Key Design Decisions

- **Zero external TUI deps**: Terminal is managed via direct platform FFI (`SetConsoleMode` on Windows, `tcsetattr` on Unix). See `term.rs` platform modules.
- **Minimal deps**: `regex = "1"` + `memchr = "2"` (memchr is already a transitive dep of regex) — no clap, crossterm, walkdir, or rayon.
- **Flicker-free rendering**: `term::paint()` uses cursor-home + per-line clear-to-EOL + clear-to-EOS in a single locked stdout write. Never use `CLEAR_SCREEN` (`\x1b[2J`).
- **UTF-8 safe slicing**: Horizontal offset uses `char_indices().nth()` — never byte-index into display strings (see `ui.rs`).
- **Atomic replacement**: `replace_in_file()` writes to a temp file (`.gref_tmp_*`) then renames over the original.
- **Parallel search**: Pipelined walk+search — `walk_and_dispatch` sends `(PathBuf, u64)` to a job channel as files are discovered; worker threads start searching immediately via `Arc<Mutex<Receiver>>`. Workers accumulate results locally in `Vec` and return them (no result channel).
- **Bytes-level regex**: `search.rs` uses `regex::bytes::Regex` (from the `regex` crate, no extra dep). Only matching lines are converted to UTF-8 via `from_utf8_lossy`. Non-matching lines skip UTF-8 conversion entirely.
- **Whole-buffer regex search**: `search_file()` feeds the entire file buffer to `pattern.find_iter(content)` instead of iterating line-by-line. This lets the regex engine's internal SIMD/Teddy/Aho-Corasick optimizations work on the full buffer. Line boundaries are resolved only for matching lines using SIMD-accelerated `memchr::memrchr` (backward) and `memchr::memchr` (forward). Incremental line counting uses `memchr::memchr_iter`.
- **SIMD literal prefilter**: `extract_longest_literal()` finds the longest ≥3-char literal substring in a regex pattern. A `memchr::memmem::Finder` is pre-built once (via `into_owned()` for `'static` lifetime), shared across worker threads via `Arc`, and used for whole-file quick-reject before engaging the regex engine.
- **Skip strategy**: Hidden files/dirs (name starts with `.`) are skipped by default. `.gitignore`, `.ignore`, and `.grefignore` files are parsed and applied hierarchically — ancestor ignore files up to the repo root are loaded at walk start, per-directory ignore files are merged during walk via `merge_dir()`. `--hidden` includes hidden items, `--no-ignore` disables ignore-file parsing. See `gitignore.rs`.
- **Zero-copy path filtering**: During directory walk, OsStr-based checks (hidden prefix, SKIP_DIRS binary search, gitignore basename match) run on `entry.file_name()` before `entry.path()` allocates the full PathBuf. Files that will be discarded never trigger path allocation.
- **Deferred binary detection**: Known extensions are classified without I/O. Files with unknown extensions are dispatched to workers and binary-checked via SIMD-accelerated `memchr(0, ...)` on the first 512 bytes of the already-loaded buffer — no separate file open.
- **Avoid allocations in hot paths**: `exclude.rs` uses `Cow` for path normalization (only allocates on Windows when backslashes present). `filedetect.rs` uses `String::with_capacity` + `push` instead of `format!`. `ui.rs` uses `str::match_indices()` instead of compiling a regex per render frame.

## Build & Test

```powershell
cargo build                    # dev build
cargo build --release          # release (strip=true, lto=true, opt-level=3)
cargo test                     # 41 unit + 98 stress/edge-case tests
cargo clippy                   # must pass with 0 warnings
.\make.ps1                     # cross-compile to dist/ (linux/darwin/windows amd64)
```

## Test Structure

- **Unit tests**: Inline `#[cfg(test)] mod tests` in `cli.rs`, `exclude.rs`, `filedetect.rs`, `gitignore.rs`, `search.rs`, `replace.rs`
- **Integration/stress tests**: `tests/stress_tests.rs` — uses `gref::` (lib) imports, covers all modules including UI rendering and key-handling simulation
- Tests use `std::env::temp_dir()` for file I/O with `gref_stress_*` / `gref_test_*` prefixed filenames
- `make_result(file, line_num, line_text)` helper creates `SearchResult` in tests (3 args, no match_text)

## Search Engine Internals

- **"Thou Shalt Not Search Line By Line"**: Feeding regex one line at a time defeats its internal SIMD optimizations. Always search the whole buffer at once with `find_iter()`, then resolve line boundaries only for matches.
- `SKIP_DIRS`: 14 hardcoded dirs skipped during walk (`.git`, `node_modules`, `target`, `.venv`, etc.) — sorted for `binary_search`
- `walk_and_dispatch`: Uses `entry.file_type()` from `read_dir` (avoids redundant stat syscalls vs `path.is_dir()`). Skips hidden entries when `skip_hidden=true`. Loads `.gitignore`, `.ignore`, `.grefignore` per directory via `merge_dir()` and checks rules via `GitIgnore::is_ignored()`. Stack carries `Arc<GitIgnore>` — `Arc::clone` is O(1) when no ignore files found. OsStr-based checks run before `entry.path()` to avoid allocation for discarded entries.
- `gitignore.rs`: Glob-to-regex conversion handles `*`, `**`, `?`, `[...]`, `\`. Basename-only matching (patterns with `/` in middle are skipped). Negation via `!`, dir-only via trailing `/`. `load_ancestor_gitignores()` walks up from root to repo root (`.git` dir) loading `.gitignore`, `.ignore`, `.grefignore` at each level. Hierarchical merging: parent rules first, child rules last (last match wins). Priority order: `.gitignore` < `.ignore` < `.grefignore`.
- `search_file`: Unified search function — reads file into memory, whole-file literal reject via `memmem::Finder` (SIMD), whole-buffer `find_iter()`, SIMD line boundary detection via `memchr`/`memrchr`, incremental line counting via `memchr_iter`, dedup matches on same line
- `MAX_FILE_SIZE`: 256 MB — files larger than this are skipped entirely
- `memmem::Finder` is pre-built once with `into_owned()` for `'static` lifetime, wrapped in `Arc` for thread sharing
- No separate small/large file paths — single unified `search_file` for all file sizes

## Conventions

- All public API in modules; `lib.rs` re-exports, `main.rs` consumes via `use gref::*`
- Error handling: `Result<(), String>` for fallible ops; `eprintln!` + `process::exit(1)` only in `main()`
- No `unwrap()` in library code paths that touch user files — propagate errors
- Style helpers in `term.rs` (`style_red`, `style_green`, etc.) wrap text with ANSI codes + RESET
- `Model` is the single source of truth — all state mutation happens in `app.rs` key handlers; `ui.rs` is pure rendering
- When adding new file extensions, maintain sorted order in `filedetect.rs` arrays (binary search)
- Prefer `Cow` / non-allocating checks over `format!` / `String` in hot paths
- Keep clippy at 0 warnings — use `strip_suffix`/`strip_prefix` instead of manual slicing, collapse nested ifs
