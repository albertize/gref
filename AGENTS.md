# AGENTS Instructions — gref

## Project Overview

A single-binary terminal TUI for search-and-replace across directory trees. Searches are literal by default; regex syntax is opt-in via `--regex`. No TUI framework — raw ANSI escapes + platform FFI for terminal control.

## Architecture

```
main.rs          → CLI parse, pattern compile, search, model init, app::run()
lib.rs           → pub mod re-exports (enables integration tests)
cli.rs           → Manual arg parsing (no clap). CliArgs struct. Flags: -v/--version, -i, -r/--regex, -e, --hidden, --no-ignore.
model.rs         → SearchResult { file_path, line_num, line_text }, AppState, AppMode, Model
search.rs        → Pipelined file walking + parallel bytes::Regex search, literal prefilter, hidden/gitignore skipping
replace.rs       → Atomic file replacement via temp file + rename
term.rs          → Raw mode FFI (Windows kernel32 / Unix termios), ANSI escapes, Key enum, paint()
ui.rs            → Screen rendering into a String (render() → term::paint())
app.rs           → Event loop: read_key → dispatch → render cycle
integration.rs   → Editor integration helpers (Vim result-file writer)
exclude.rs       → Path exclusion (dir/, *.ext, exact filename patterns). Uses Cow for path normalization.
filedetect.rs    → Text vs binary detection (64 text extensions + 512-byte content probe + SIMD null-byte scan)
gitignore.rs     → .gitignore/.ignore/.grefignore parsing (glob→regex), hierarchical rule merging, ancestor discovery
```

**Data flow:** `cli::parse()` → `search::compile_search_pattern()` → `search::perform_search_adaptive()` → `Model::new()` → `app::run()` (loop: `ui::render()` → `term::paint()` → `term::read_key()` → state update) → `replace::perform_replacements_with_options()`.

## Key Design Decisions

- **Zero external TUI deps**: Terminal is managed via direct platform FFI (`SetConsoleMode` on Windows, `tcsetattr` on Unix). See `term.rs` platform modules.
- **Unix raw-mode invariant**: `term.rs` still uses hard-coded `termios` byte offsets. On Linux, `c_cc[VTIME]` is byte 22 and `c_cc[VMIN]` is byte 23; raw mode must keep `VTIME=1`, `VMIN=0` so lone `Esc` resolves via timeout instead of blocking the event loop.
- **Minimal deps**: `regex = "1"` + `memchr = "2"` (memchr is already a transitive dep of regex) — no clap, crossterm, walkdir, or rayon.
- **Flicker-free rendering**: `term::paint()` uses cursor-home + per-line clear-to-EOL + clear-to-EOS in a single locked stdout write. Never use `CLEAR_SCREEN` (`\x1b[2J`).
- **UTF-8 safe slicing**: Horizontal offset uses `char_indices().nth()` — never byte-index into display strings (see `ui.rs`).
- **Literal default / regex opt-in**: `search::compile_search_pattern()` escapes user patterns unless `--regex` is set. In literal mode, replacement text is literal too, so `$1` is written as `$1`; capture expansion is a regex-mode behavior.
- **Compiled-pattern-consistent UI highlighting**: `ui.rs` must render highlights and selected replacement previews with `Model.pattern`, not raw `pattern_str`, so regex searches such as `1.2.0` visibly mark the actual match.
- **Atomic replacement**: `replace_in_file()` writes to a temp file (`.gref_tmp_*`) then renames over the original.
- **Bounded byte-preserving replacement**: `replace.rs` rewrites selected lines with `regex::bytes::Regex` semantics, preserving non-UTF-8 bytes outside matches and streaming output directly to the temp file. Selected lines are buffered only up to `MAX_REPLACE_LINE_BYTES` (64 MiB); larger selected lines fail cleanly instead of aborting on allocator growth.
- **Parallel search**: Pipelined walk+search — `walk_and_dispatch` sends `(PathBuf, u64)` to a job channel as files are discovered; worker threads start searching immediately via `Arc<Mutex<Receiver>>`. Workers accumulate results locally in `Vec` and return them (no result channel).
- **Bytes-level regex engine**: `search.rs` uses `regex::bytes::Regex` internally (from the `regex` crate, no extra dep), including escaped literal patterns. Only matching lines are converted to UTF-8 via `from_utf8_lossy`. Non-matching lines skip UTF-8 conversion entirely.
- **Whole-buffer regex search**: `search_file()` feeds the entire file buffer to `pattern.find_iter(content)` instead of iterating line-by-line. This lets the regex engine's internal SIMD/Teddy/Aho-Corasick optimizations work on the full buffer. Line boundaries are resolved only for matching lines using SIMD-accelerated `memchr::memrchr` (backward) and `memchr::memchr` (forward). Incremental line counting uses `memchr::memchr_iter`.
- **SIMD literal prefilter**: `extract_longest_literal()` finds the longest ≥3-char literal substring in a regex pattern. A `memchr::memmem::Finder` is pre-built once (via `into_owned()` for `'static` lifetime), shared across worker threads via `Arc`, and used for whole-file quick-reject before engaging the regex engine.
- **Skip strategy**: Hidden files/dirs (name starts with `.`) are skipped by default outside Git repo roots. When the search root contains a `.git` directory, `search::default_skip_hidden()` includes hidden items by default so paths such as `.github/` are searchable; `.git` itself is still always skipped via `SKIP_DIRS`. The walker also detects nested Git repo roots while descending from a non-repo parent and stops skipping hidden entries from that repo root downward. `.gitignore`, `.ignore`, and `.grefignore` files are parsed and applied hierarchically — ancestor ignore files up to the repo root are loaded at walk start, per-directory ignore files are merged during walk via `merge_dir()`. `--hidden` includes hidden items, `--no-ignore` disables ignore-file parsing. See `gitignore.rs`.
- **Zero-copy path filtering**: During directory walk, OsStr-based checks (hidden prefix, SKIP_DIRS binary search, gitignore basename match) run on `entry.file_name()` before `entry.path()` allocates the full PathBuf. Files that will be discarded never trigger path allocation.
- **Deferred binary detection**: Known extensions are classified without I/O. Files with unknown extensions are dispatched to workers and binary-checked via SIMD-accelerated `memchr(0, ...)` on the first 512 bytes of the already-loaded buffer — no separate file open.
- **Vim integration**: `--vim-result <file>` enables Vim popup hosting. `main.rs` writes a small atomic status protocol via `integration.rs`: `selected\nline\ncolumn\npath`, `none`, `error\nmessage`, `replaced`, or `cancelled`. Vim runtime files live in `contrib/vim/` and use built-in `term_start()` + `popup_create()` only. In Vim-hosted mode, `Enter` opens a selected search result and the `v` external-editor key is disabled.
- **Avoid allocations in hot paths**: `exclude.rs` uses `Cow` for path normalization (only allocates on Windows when backslashes present). `filedetect.rs` uses `String::with_capacity` + `push` instead of `format!`. `ui.rs` uses `str::match_indices()` instead of compiling a regex per render frame.

## Build & Test

```bash
cargo build                    # dev build
cargo build --release          # release (strip=true, lto=true, opt-level=3)
cargo test                     # unit + stress/edge-case + Vim runtime tests
cargo clippy                   # must pass with 0 warnings
```

## Test Structure

- **Unit tests**: Inline `#[cfg(test)] mod tests` in `cli.rs`, `exclude.rs`, `filedetect.rs`, `gitignore.rs`, `search.rs`, `replace.rs`
- **Integration/stress tests**: `tests/stress_tests.rs` — uses `gref::` (lib) imports, covers all modules including UI rendering and key-handling simulation
- **Vim runtime test**: `tests/vim_runtime_tests.rs` launches `vim -Nu NONE -n -es` to validate Vimscript parsing and result protocol; skips cleanly when `vim` is unavailable
- Tests use `std::env::temp_dir()` for file I/O with `gref_stress_*` / `gref_test_*` prefixed filenames
- `make_result(file, line_num, line_text)` helper creates `SearchResult` in tests (3 args, no match_text)

## Search Engine Internals

- **"Thou Shalt Not Search Line By Line"**: Feeding regex one line at a time defeats its internal SIMD optimizations. Always search the whole buffer at once with `find_iter()`, then resolve line boundaries only for matches.
- `SKIP_DIRS`: 14 hardcoded dirs skipped during walk (`.git`, `node_modules`, `target`, `.venv`, etc.) — sorted for `binary_search`
- `walk_and_dispatch`: Uses `entry.file_type()` from `read_dir` (avoids redundant stat syscalls vs `path.is_dir()`). Skips hidden entries when `skip_hidden=true`; CLI callers derive this via `search::default_skip_hidden()`, which disables hidden skipping for roots containing `.git`. Loads `.gitignore`, `.ignore`, `.grefignore` per directory via `merge_dir()` and checks rules via `GitIgnore::is_ignored()`. Stack carries `Arc<GitIgnore>` — `Arc::clone` is O(1) when no ignore files found. OsStr-based checks run before `entry.path()` to avoid allocation for discarded entries.
- `gitignore.rs`: Glob-to-regex conversion handles `*`, `**`, `?`, `[...]`, `\`. Basename-only matching (patterns with `/` in middle are skipped). Negation via `!`, dir-only via trailing `/`. `load_ancestor_gitignores()` walks up from root to repo root (`.git` dir) loading `.gitignore`, `.ignore`, `.grefignore` at each level. Hierarchical merging: parent rules first, child rules last (last match wins). Priority order: `.gitignore` < `.ignore` < `.grefignore`.
- `search_file`: Unified search function — reads file into memory, whole-file literal reject via `memmem::Finder` (SIMD), whole-buffer `find_iter()`, SIMD line boundary detection via `memchr`/`memrchr`, incremental line counting via `memchr_iter`, dedup matches on same line
- `perform_search_adaptive`: Accepts either a directory root or an explicit single-file root. Single-file roots reuse the same whole-buffer `search_file()` path and are used by `:GrefBuffer`.
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

## Repo Memory

- Treat this file as the repo's durable memory.
- When you learn a new repo-specific fact that is likely to matter for future work, add it here or fold it into the most relevant existing section.
- Keep additions compact and high-signal: record only important architecture, invariants, workflows, performance constraints, or test conventions.
- Do not add temporary notes, one-off debugging details, obvious observations, or duplicate information.
