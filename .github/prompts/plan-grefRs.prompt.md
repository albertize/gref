# gref-rs — Implementation Plan

> Rust port of [gref](../gref/) with minimal dependencies.  
> **Only crate: `regex`**. Everything else via `std` + ANSI escapes + platform FFI.

---

## Table of Contents

1. [Project Scaffolding](#step-1-project-scaffolding)
2. [Data Model (`model.rs`)](#step-2-data-model)
3. [CLI Parsing (`cli.rs`)](#step-3-cli-parsing)
4. [Exclude Logic (`exclude.rs`)](#step-4-exclude-logic)
5. [File Detection (`filedetect.rs`)](#step-5-file-detection)
6. [Search Engine (`search.rs`)](#step-6-search-engine)
7. [Replace Engine (`replace.rs`)](#step-7-replace-engine)
8. [Terminal Abstraction (`term.rs`)](#step-8-terminal-abstraction)
9. [TUI Rendering (`ui.rs`)](#step-9-tui-rendering)
10. [Event Loop & State Machine (`app.rs`)](#step-10-event-loop--state-machine)
11. [Wire Everything in `main.rs`](#step-11-main)
12. [Tests](#step-12-tests)
13. [Build Scripts & CI](#step-13-build-scripts)

---

## Step 1 — Project Scaffolding

Create the Cargo project and directory layout.

### Files to create

```
gref-rs/
├── Cargo.toml
├── .gitignore
├── README.md
├── makefile
├── make.ps1
└── src/
    ├── main.rs
    ├── cli.rs
    ├── model.rs
    ├── exclude.rs
    ├── filedetect.rs
    ├── search.rs
    ├── replace.rs
    ├── term.rs
    ├── ui.rs
    └── app.rs
```

### `Cargo.toml`

- `[package]` name = `"gref"`, edition = `"2021"`.
- `[dependencies]`: only `regex = "1"`.
- `[profile.release]`: `strip = true`, `lto = true`, `opt-level = "z"` (small binary, matches Go's `-ldflags="-s -w"`).

### `.gitignore`

Copy from the Go project, replacing Go-specific entries with Rust ones:
- `/target/`
- `/dist/`
- `*.pdb`
- `.env`
- `.idea/`
- `.vscode/`

### `main.rs` (stub)

Declare all modules (`mod cli; mod model; ...`), call `cli::parse()`, print parsed args, and exit.  
Ensures the project compiles from the start.

---

## Step 2 — Data Model (`model.rs`)

Port `core/model.go` data structures (not the TUI logic — that goes in `app.rs` and `ui.rs`).

### Types to define

```
SearchResult { file_path: String, line_num: usize, line_text: String, match_text: String }

AppState enum: Browse, Confirming, Replacing, Done
AppMode  enum: Default, SearchOnly

Model struct {
    results:           Vec<SearchResult>,
    cursor:            usize,
    topline:           usize,
    screen_height:     usize,
    screen_width:      usize,
    selected:          HashSet<usize>,    // indices into results
    pattern:           Regex,
    pattern_str:       String,
    replacement_str:   String,
    mode:              AppMode,
    state:             AppState,
    error:             Option<String>,
    horizontal_offset: usize,
}
```

### Functions

- `Model::new(results, pattern_str, replacement_str, pattern, mode) -> Model`  
  Mirrors `InitModel`. Sets `screen_height = 20`, `screen_width = 80`, empty selected set, state = `Browse`.

- Implement `Default` for `AppState` (→ `Browse`) and `AppMode` (→ `Default`).

### Notes

- `SearchResult` derives `Clone` (needed when grouping by file for replacement).
- `Model` owns all data — no lifetimes needed.

---

## Step 3 — CLI Parsing (`cli.rs`)

Port the `flag`-based CLI from `main.go` using only `std::env::args()`.

### Struct

```
CliArgs {
    pattern:      String,
    replacement:  Option<String>,   // None → SearchOnly mode
    root_path:    String,           // default "."
    ignore_case:  bool,             // -i / --ignore-case
    exclude:      Vec<String>,      // parsed from -e / --exclude
    show_help:    bool,             // -h / --help
}
```

### Parsing logic

1. Collect `std::env::args().skip(1)` into a `Vec<String>`.
2. Iterate with an index. For each element:
   - `"-h"` | `"--help"` → set `show_help = true`.
   - `"-i"` | `"--ignore-case"` → set `ignore_case = true`.
   - `"-e"` | `"--exclude"` → consume **next** element as the comma-separated exclude string.
   - Anything starting with `-` that's unknown → print error and `std::process::exit(1)`.
   - Otherwise → push into a `positional: Vec<String>`.
3. Map positionals: `[0]` = pattern (required), `[1]` = replacement, `[2]` = root_path.
4. If `show_help`, print the help text (same content as Go version) and exit.

### `parse_exclude_list(s: &str) -> Vec<String>`

Split on `','`, trim whitespace, filter empty, collect.  
This is a standalone public function (also used in tests).

### Help text

Embed the exact same help text from Go's `main.go` as a `const HELP_TEXT: &str`.

---

## Step 4 — Exclude Logic (`exclude.rs`)

Port `IsExcluded` from `core/search.go`.

### `is_excluded(path: &str, exclude_list: &[String]) -> bool`

1. Normalize `path` to forward slashes (`path.replace('\\', "/")`).
2. For each pattern in `exclude_list`:
   - If pattern ends with `"/"` → check `normalized.contains(pattern)` or `(normalized + "/").ends_with(pattern)`.
   - If pattern starts with `"*."` → check `normalized.ends_with(&pattern[1..])`.
   - Otherwise → extract filename component (last segment after `/`) and compare `== pattern`.
3. Return `true` on first match.

### Notes

- The Go code uses `filepath.ToSlash` — in Rust we do manual `replace('\\', '/')`.
- `filepath.Base` equivalent: split on `/` and take last, or use `std::path::Path::file_name()` then convert to `&str`.

---

## Step 5 — File Detection (`filedetect.rs`)

Port `isLikelyTextFile`, `isTextFileContent`, and the extension maps from `core/search.go`.

### Constants

Define two `&[&str]` arrays (or use `HashSet` built with `once_cell` pattern or a function):

- `TEXT_EXTENSIONS`: `.txt`, `.go`, `.py`, `.js`, `.ts`, `.java`, `.cpp`, `.c`, `.h`, `.hpp`, `.cs`, `.php`, `.rb`, `.rs`, `.html`, `.css`, `.xml`, `.json`, `.yaml`, `.yml`, `.md`, `.rst`, `.sh`, `.bat`, `.ps1`, `.conf`, `.cfg`, `.ini`
- `BINARY_EXTENSIONS`: all extensions from the Go `binaryExtensions` map (images, archives, audio, video, databases, fonts, certs, compiled files, etc.)

Use a function `is_known_text(ext) -> bool` and `is_known_binary(ext) -> bool` that do a simple `.contains()` on the arrays. Alternatively, use `phf` — but since we want zero crates beyond `regex`, use sorted arrays + `binary_search` for O(log n).

### `is_likely_text_file(path: &Path) -> bool`

1. Extract extension (lowercase): `path.extension()?.to_str()?.to_lowercase()`.
2. If in `TEXT_EXTENSIONS` → `true`.
3. If in `BINARY_EXTENSIONS` → `false`.
4. Otherwise → `is_text_file_content(path)`.

### `is_text_file_content(path: &Path) -> bool`

1. Open file, read up to 512 bytes into a `[u8; 512]` buffer.
2. For each byte `b`:
   - If `b == 0` or (`b < 32` and `b != 9` and `b != 10` and `b != 13`) → return `false`.
3. Return `true`.

---

## Step 6 — Search Engine (`search.rs`)

Port `PerformSearchAdaptive` and all search helpers from `core/search.go`.

### Helper functions

#### `extract_literal_prefix(regex_str: &str) -> Option<String>`

Same logic as Go:
1. If contains `"(?i)"` → return `None`.
2. Strip leading `^`, `(?m)`, trailing `$`.
3. Walk chars: on `\` set escaped flag, on metachar `.*+?^$()[]{}|` → stop, else accumulate.
4. Return `Some(result)` only if `result.len() >= 3`, else `None`.

#### `search_lines(path: &str, content: &[u8], pattern: &Regex) -> Vec<SearchResult>`

1. Split content by `\n` (preserving logic — use `content.split(|&b| b == b'\n')`).
2. For each line, convert to `&str` (use `String::from_utf8_lossy` for safety).
3. If `pattern.is_match(line_str)`:
   - `pattern.find(line_str)` → first match text.
   - Push `SearchResult { file_path, line_num, line_text: line_str.to_string(), match_text }`.

#### `search_small_file(path: &str, pattern: &Regex) -> Vec<SearchResult>`

1. `std::fs::read(path)` → full content.
2. Quick-reject: `pattern.is_match(&content_str)` on the whole file.
3. If matches → `search_lines(path, &content, pattern)`.

#### `search_large_file(path: &str, pattern: &Regex) -> Vec<SearchResult>`

1. Open with `BufReader`, iterate with `.lines()` (but to handle non-UTF8, use `read_line` into a buffer or read bytes manually).
2. Actually, use `BufRead::read_until(b'\n', &mut buf)` to handle arbitrary bytes:
   - Convert each line buffer to `String` via `String::from_utf8_lossy`.
   - Test with `pattern.is_match(&line)`.
3. Threshold: file size > 10 MB.

#### `search_with_prefilter(path: &str, pattern: &Regex, literal: &str) -> Vec<SearchResult>`

1. Read full file as bytes.
2. Fast-reject: check if `content` contains `literal.as_bytes()` (use a simple `windows().any()` search or `memchr`-style manual scan — `std` doesn't have `memmem`, so implement a naive byte search, or use `content.windows(literal.len()).any(|w| w == literal.as_bytes())`.
3. Then regex quick-reject on whole content.
4. Then `search_lines`.

### Main function: `perform_search_adaptive`

```
pub fn perform_search_adaptive(
    root_path: &str,
    pattern: &Regex,
    exclude_list: &[String],
) -> Result<Vec<SearchResult>, String>
```

1. Compute `literal = extract_literal_prefix(pattern.as_str())`.
2. Collect files by walking the directory tree:
   - Use `std::fs::read_dir` recursively (implement a simple recursive function or use a stack-based iterative walker).
   - **Skip directories**: `.git`, `.cache`, `node_modules` (hardcoded) + `is_excluded()` check.
   - **Skip files**: `is_excluded()` check + `!is_likely_text_file()`.
   - Collect as `Vec<(PathBuf, u64)>` (path + file size from metadata).
3. **Parallelism** using `std::thread` + `std::sync::mpsc`:
   - Determine worker count: `std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)`.
   - Create a channel pair for jobs (`Sender<(PathBuf, u64)>` / `Receiver`) and a channel for results (`Sender<Vec<SearchResult>>` / `Receiver`).
   - Spawn `n` worker threads. Each worker loops on `job_rx.recv()`:
     - If `size > 10 MB` → `search_large_file`.
     - Else if `literal.is_some()` → `search_with_prefilter`.
     - Else → `search_small_file`.
     - Send non-empty results through `result_tx`.
   - Producer: iterate the collected file list, send each as a job.
   - Drop the job sender to signal completion.
   - Collect all results from `result_rx` into a single `Vec<SearchResult>`.

### Notes on `Regex` sharing across threads

`regex::Regex` is `Send + Sync`, so it can be shared via `Arc<Regex>`. Clone the `Arc` into each worker thread.

For the exclude list and literal prefix, clone them or wrap in `Arc`.

---

## Step 7 — Replace Engine (`replace.rs`)

Port `PerformReplacements` and `ReplaceInFile` from `core/replace.go`.

### `perform_replacements`

```
pub fn perform_replacements(
    all_results: &[SearchResult],
    selected: &HashSet<usize>,
    pattern: &Regex,
    replacement: &str,
) -> Result<(), String>
```

1. Group selected results by `file_path` → `HashMap<&str, Vec<&SearchResult>>`.
2. For each file group, call `replace_in_file`.

### `replace_in_file`

```
pub fn replace_in_file(
    file_path: &str,
    results: &[&SearchResult],
    pattern: &Regex,
    replacement: &str,
) -> Result<(), String>
```

1. Build `HashSet<usize>` of line numbers to replace.
2. Open source file with `BufReader`.
3. Create temp file in the **same directory** as source:
   - Use `std::env::temp_dir()` — NO, must be same dir for atomic rename.
   - Generate a name: `format!("gref_tmp_{}", std::process::id())` or use a counter, or use `tempfile`-like logic manually.
   - Actually, to be safe: use the pattern `{dir}/gref_tmp_{pid}_{counter}` — increment a static atomic counter.
   - Or simpler: `{dir}/.gref_tmp_{random}` where random = `std::time::SystemTime::now().duration_since(UNIX_EPOCH).as_nanos()`.
4. Read line-by-line using `BufRead::read_until(b'\n', &mut buf)`:
   - This preserves `\r\n` (the `\r` stays in the buffer before `\n`).
   - Increment `line_num` counter.
   - If `line_num` is in the replace set:
     - Convert to string: `String::from_utf8_lossy(&buf)`.
     - Apply `pattern.replace_all(&line, replacement)`.
     - Write result bytes to temp file.
   - Else: write `buf` bytes as-is to temp file.
5. Flush the `BufWriter` wrapping the temp file.
6. Drop (close) both file handles.
7. `std::fs::rename(tmp_path, file_path)` — atomic on same filesystem.
8. On any error: attempt `std::fs::remove_file(tmp_path)` in a cleanup block.

### Edge cases to handle

- **Empty file**: `read_until` returns `Ok(0)` immediately → loop body never executes → temp file is empty → rename succeeds.
- **Last line without `\n`**: `read_until` returns the remaining bytes without a trailing `\n` — this is handled correctly because we don't require `\n`.
- **Non-UTF-8 bytes**: `from_utf8_lossy` replaces invalid sequences with `U+FFFD`, but for the replacement line we need to be careful. Alternative: try `from_utf8` first, on failure write the line as-is (no replacement on broken lines). This matches the Go behavior where `pattern.ReplaceAllString` operates on Go strings (which can contain arbitrary bytes).
- **Permissions**: propagate errors via `Result`.

---

## Step 8 — Terminal Abstraction (`term.rs`)

This is the most complex step. We replace `bubbletea` + `lipgloss` with raw ANSI escapes and platform-specific raw mode.

### ANSI Escape Sequences (constants)

```
const ESC: &str = "\x1b";
const ALT_SCREEN_ON:  &str = "\x1b[?1049h";
const ALT_SCREEN_OFF: &str = "\x1b[?1049l";
const CURSOR_HIDE:    &str = "\x1b[?25l";
const CURSOR_SHOW:    &str = "\x1b[?25h";
const CLEAR_SCREEN:   &str = "\x1b[2J";
const CURSOR_HOME:    &str = "\x1b[H";
const RESET:          &str = "\x1b[0m";
const BOLD:           &str = "\x1b[1m";
```

Color functions:
```
fn fg_color(ansi256: u8) -> String  → format!("\x1b[38;5;{}m", ansi256)
fn bg_color(ansi256: u8) -> String  → format!("\x1b[48;5;{}m", ansi256)
```

Named style helpers matching Go's lipgloss colors:
```
fn style_red(text: &str) -> String       → fg_color(9) + text + RESET    // ColorRed = 9
fn style_green(text: &str) -> String     → fg_color(10) + text + RESET   // ColorGreen = 10
fn style_cyan_bold(text: &str) -> String → BOLD + fg_color(6) + text + RESET  // ColorCyan = 6
fn style_grey(text: &str) -> String      → fg_color(240) + text + RESET  // ColorGrey = 240
fn style_red_bold(text: &str) -> String  → BOLD + fg_color(9) + text + RESET  // error style
```

### Raw mode (platform-specific)

#### Unix (`#[cfg(unix)]`)

```rust
use std::os::unix::io::AsRawFd;

extern "C" {
    fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, action: i32, termios: *const Termios) -> i32;
}
```

Define a minimal `Termios` struct matching the C layout (fields: `c_iflag`, `c_oflag`, `c_cflag`, `c_lflag`, `c_cc: [u8; 20]`, speed fields). The exact layout varies by OS — use `#[repr(C)]` and size the `c_cc` array appropriately:
- Linux: `c_cc` has 32 elements, struct also has `c_ispeed` and `c_ospeed`.
- macOS: `c_cc` has 20 elements, struct also has `c_ispeed` and `c_ospeed`.

To keep it simple and correct across platforms, use a **byte array** approach:
```rust
#[repr(C)]
struct RawTermios([u8; 256]); // oversized but safe — zeroed on init
```

Then `tcgetattr` fills it, we modify the relevant bytes (ICANON and ECHO bits in `c_lflag`), and `tcsetattr` writes it back. Alternatively, just define the correct struct for each target with `#[cfg(target_os = ...)]`.

**Simplified approach**: define `enable_raw_mode()` and `disable_raw_mode()` that save/restore termios:

```rust
static ORIG_TERMIOS: Mutex<Option<Vec<u8>>> = Mutex::new(None);

pub fn enable_raw_mode() { ... }
pub fn disable_raw_mode() { ... }
```

The `c_lflag` modifications to clear: `ECHO`, `ICANON`, `ISIG`, `IEXTEN`.  
The `c_iflag` modifications to clear: `IXON`, `ICRNL`, `BRKINT`, `INPCK`, `ISTRIP`.  
Set `c_oflag` to keep `OPOST` (so `\n` still works as newline).  
Set `c_cc[VMIN] = 0`, `c_cc[VTIME] = 1` (100ms timeout for non-blocking reads).

#### Windows (`#[cfg(windows)]`)

Use `std::os::windows::io::AsRawHandle` on stdin and direct FFI to `kernel32.dll`:

```rust
extern "system" {
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: *mut std::ffi::c_void, mode: u32) -> i32;
    fn GetStdHandle(std_handle: u32) -> *mut std::ffi::c_void;
}

const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6; // -10 as u32
const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5; // -11 as u32
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
const ENABLE_LINE_INPUT: u32 = 0x0002;
const ENABLE_ECHO_INPUT: u32 = 0x0004;
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
```

`enable_raw_mode()`:
1. Get stdin handle via `GetStdHandle(STD_INPUT_HANDLE)`.
2. `GetConsoleMode` → save original mode.
3. Clear `ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT`.
4. Set `ENABLE_VIRTUAL_TERMINAL_INPUT`.
5. `SetConsoleMode` with new mode.
6. Also enable `ENABLE_VIRTUAL_TERMINAL_PROCESSING` on stdout handle (for ANSI escape output support).

`disable_raw_mode()`: restore saved mode.

### Terminal size

#### Unix
```rust
extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

#[repr(C)]
struct Winsize { ws_row: u16, ws_col: u16, ws_xpixel: u16, ws_ypixel: u16 }

// TIOCGWINSZ = 0x5413 on Linux, 0x40087468 on macOS
```

#### Windows
```rust
extern "system" {
    fn GetConsoleScreenBufferInfo(handle: *mut c_void, info: *mut ConsoleScreenBufferInfo) -> i32;
}
```

Provide a unified function:
```
pub fn terminal_size() -> (u16, u16)  // (cols, rows)
```

### Key reading

Read from stdin byte-by-byte (or small buffer). Parse escape sequences:

```
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Space,
    CtrlC,
    Unknown,
}

pub fn read_key() -> Option<Key>
```

Logic:
1. Read one byte from stdin.
   - If `Ok(0)` or `Err` with timeout → return `None` (no input available).
2. Match:
   - `0x03` → `CtrlC`
   - `0x0D` (or `0x0A` on Unix) → `Enter`
   - `0x1B` → start of escape sequence:
     - Read next byte. If `[` → CSI sequence:
       - `A` → `Up`, `B` → `Down`, `C` → `Right`, `D` → `Left`
       - `H` → `Home`, `F` → `End`
       - `1~` → `Home`, `4~` → `End` (alternate encoding)
     - If nothing follows within timeout → `Escape`
   - `0x20` → `Space`
   - `b'q'` → `Char('q')`
   - `b'j'` → `Char('j')`, etc.
   - Other printable byte → `Char(byte as char)`.

On **Unix**: read from fd 0 with the `VTIME` timeout set.  
On **Windows**: use `ReadConsoleInputW` or simply `std::io::stdin().read()` (which works in raw mode with VT input enabled).

### Public API summary for `term.rs`

```
pub fn enable_raw_mode()
pub fn disable_raw_mode()
pub fn enter_alt_screen()     // print ALT_SCREEN_ON + CURSOR_HIDE
pub fn leave_alt_screen()     // print CURSOR_SHOW + ALT_SCREEN_OFF
pub fn terminal_size() -> (u16, u16)
pub fn clear_and_home()       // print CLEAR_SCREEN + CURSOR_HOME
pub fn read_key() -> Option<Key>
pub fn style_red(s: &str) -> String
pub fn style_green(s: &str) -> String
pub fn style_cyan_bold(s: &str) -> String
pub fn style_grey(s: &str) -> String
pub fn style_red_bold(s: &str) -> String
pub fn style_bold(s: &str) -> String
```

---

## Step 9 — TUI Rendering (`ui.rs`)

Port the `View()`, `headerView()`, and `footerView()` methods from `core/model.go`.

### `pub fn render(model: &Model) -> String`

Build the full screen content as a single `String`, then print it in one `write!` call (minimizes flicker).

1. Call `render_header(model)`.
2. If `state == Browse`:
   - Build `visible_lines` list (same logic as Go): iterate `model.results`, insert a file header line (`"DIR: {path}"`) whenever `file_path` changes.
   - Find `cursor_line` index in `visible_lines`.
   - Adjust `model.topline` so cursor is visible.
   - For each visible line in the window `[topline .. topline + screen_height]`:
     - **File header**: `"DIR: {file_path}\n"` — no styling.
     - **Result line**: `"{cursor_indicator}{checkbox} {line_num}: {styled_text}\n"`
       - `cursor_indicator`: `"> "` (bold) if current, else `"  "`.
       - `checkbox`: `"[x]"` (cyan bold) if selected, else `"[ ]"`.
       - `styled_text`: apply horizontal offset, then for each regex match:
         - If line is selected → show replacement string in cyan bold.
         - Else → show match in red.
         - Non-matching parts → unstyled.
   - Use `regex::escape(&model.pattern_str)` to create a literal-match regex for **display highlighting** (matching the Go behavior which uses `regexp.QuoteMeta`).
3. Call `render_footer(model)`.

### `fn render_header(model: &Model) -> String`

Match Go's `headerView()`:
- **Error state**: red bold error message + "Press 'q' to exit."
- **Browse + Default**: `"--- Search results (Pattern: {red pattern}) ---\nReplacing with: {green replacement}\n\n"`
- **Browse + SearchOnly**: `"--- Search results (Pattern: {red pattern}) ---\nSearch Only Mode\n\n"`
- **Confirming**: `"Replacing {count}?\nPattern: {red} -> Replace: {green}\n\n"`
- **Replacing**: `"Replacing... wait.\n"` (fix the Go typo "whait" → "wait")
- **Done**: `"Success.\n"`

### `fn render_footer(model: &Model) -> String`

- **Browse**: grey-styled line count + keybinding help (same text as Go).
- **Confirming**: grey-styled `"Enter: confirm | Esc: exit"`.
- Other states: empty.

---

## Step 10 — Event Loop & State Machine (`app.rs`)

Port the Bubble Tea `Update()` method as a manual event loop.

### `pub fn run(model: &mut Model) -> Result<(), String>`

```rust
pub fn run(model: &mut Model) -> Result<(), String> {
    term::enable_raw_mode();
    term::enter_alt_screen();

    // Install a panic hook that restores the terminal
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = term::disable_raw_mode();
        let _ = term::leave_alt_screen();
        default_hook(info);
    }));

    loop {
        // Update terminal size
        let (cols, rows) = term::terminal_size();
        model.screen_width = cols as usize;
        model.screen_height = (rows as usize).saturating_sub(10).max(1);

        // Render
        let output = ui::render(model);
        term::clear_and_home();
        print!("{}", output);
        flush stdout;

        // Read input
        if let Some(key) = term::read_key() {
            match model.state {
                AppState::Browse => handle_browse_key(model, key),
                AppState::Confirming => handle_confirming_key(model, key),
                AppState::Done | AppState::Replacing => {
                    // In Done state, any key quits (or auto-quit after replacement)
                }
            }
        }

        // Check for quit conditions
        if model.state == AppState::Done {
            break;
        }
        if model.state == AppState::Replacing {
            // Perform replacement synchronously (no async needed without a TUI framework)
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
            // Render the final "Success" or error screen
            let output = ui::render(model);
            term::clear_and_home();
            print!("{}", output);
            flush stdout;
            // Brief pause so user sees the result
            std::thread::sleep(std::time::Duration::from_millis(800));
            break;
        }
    }

    term::leave_alt_screen();
    term::disable_raw_mode();
    Ok(())
}
```

### `fn handle_browse_key(model: &mut Model, key: Key)`

Map keys exactly as Go's `Update()`:

| Key | Action |
|-----|--------|
| `CtrlC`, `Char('q')` | Set state = `Done` |
| `Up`, `Char('k')` | Decrement cursor, adjust topline |
| `Down`, `Char('j')` | Increment cursor, adjust topline |
| `Left`, `Char('h')` | Decrease `horizontal_offset` by 10, clamp to 0 |
| `Right`, `Char('l')` | Increase `horizontal_offset` by 5, clamp to max |
| `Home` | `horizontal_offset = 0` |
| `End` | `horizontal_offset = 1000` |
| `Space` | Toggle selection at cursor (only if mode != SearchOnly) |
| `Char('a')` | Select all (only if mode != SearchOnly) |
| `Char('n')` | Deselect all (only if mode != SearchOnly) |
| `Enter` | If mode != SearchOnly and selected is non-empty → state = `Confirming`. If selected is empty → set error "no results". |

### `fn handle_confirming_key(model: &mut Model, key: Key)`

| Key | Action |
|-----|--------|
| `Enter` | state = `Replacing` |
| `Escape` | state = `Browse`, clear error |
| `CtrlC`, `Char('q')` | state = `Done` |

---

## Step 11 — Wire Everything in `main.rs`

```rust
mod cli;
mod model;
mod exclude;
mod filedetect;
mod search;
mod replace;
mod term;
mod ui;
mod app;

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
        &args.root_path, &pattern, &args.exclude
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
```

---

## Step 12 — Tests

Create `src/tests/` or use `#[cfg(test)] mod tests` inside each module. Also create integration-style tests in a `tests/` directory at the crate root.

### Unit tests to port (from Go test files)

#### In `exclude.rs` (or `tests/exclude_test.rs`)

- **`test_parse_exclude_list`**: `"foo, bar ,baz/"` → `["foo", "bar", "baz/"]`.
- **`test_is_excluded`**: table-driven test with the same cases:
  - `"/home/user/project/.git"` + `[".git", "*.log", "media/", "file.txt"]` → `true`
  - `"/home/user/project/media/image.png"` → `true`
  - `"/home/user/project/file.txt"` → `true`
  - `"/home/user/project/notes.log"` → `true`
  - `"/home/user/project/notes.txt"` → `false`
  - `"/home/user/project/src/main.go"` → `false`

#### In `replace.rs` (or `tests/replace_test.rs`)

Port all 11 test cases:

| Test | Setup | Expected |
|------|-------|----------|
| `test_replace_in_file` | `"foo bar\nfoo baz\nbar foo"`, replace lines 1,2 `foo→qux` | `"qux bar\nqux baz\nbar foo"` |
| `test_replace_windows_line_endings` | `"foo bar\r\nfoo baz\r\nbar foo"`, lines 1,2 | `"qux bar\r\nqux baz\r\nbar foo"` |
| `test_replace_empty_file` | `""`, no results | `""` |
| `test_replace_only_matches` | `"foo\nfoo\nfoo"`, all lines | `"qux\nqux\nqux"` |
| `test_replace_no_matches` | `"bar\nbaz\nquux"`, no results | `"bar\nbaz\nquux"` |
| `test_replace_special_chars` | `"föö bär\nföö baz\nbär föö"`, lines 1,2, `föö→qux` | `"qux bär\nqux baz\nbär föö"` |
| `test_replace_byte_conflict` | Bytes with `0xFF`, `0xFE` | Correct replacement preserving raw bytes |
| `test_replace_file_not_readable` | Set permissions to 0o000 (Unix only, `#[cfg(unix)]`) | Expect `Err` |
| `test_replace_invalid_regexp` | `regex::Regex::new("[")` should fail | Verify `is_err()` |
| `test_replace_overlapping_match` | `"aaaaa"`, pattern `aa`, replace `b` | Non-empty result, no panic |
| `test_replace_null_bytes` | Bytes with `0x00` | Correct replacement preserving null bytes |

Each test:
1. Writes a temp file in `std::env::temp_dir()` (or use `tempfile`-like manual naming).
2. Calls `replace_in_file`.
3. Reads back and asserts content.
4. Cleans up in a `Drop` guard or explicit `std::fs::remove_file`.

### Additional tests to add (Rust-specific)

- **`filedetect.rs`**: test `is_likely_text_file` for known extensions, unknown extensions, binary content detection.
- **`search.rs`**: test `extract_literal_prefix` with various regex patterns:
  - `"hello"` → `Some("hello")`
  - `"(?i)hello"` → `None`
  - `"he.*lo"` → `Some("he")`  (only 2 chars, < 3 → `None`)
  - `"hel\\.lo"` → `Some("hel.lo")`
  - `"abc"` → `Some("abc")`
- **`cli.rs`**: test argument parsing with various input vectors.

---

## Step 13 — Build Scripts

### `makefile` (Unix)

```makefile
.PHONY: build-all clean zip-all build-local

DIST_DIR := dist
BIN := gref

build-all: clean
	@echo "Building for linux-amd64..."
	cargo build --release --target x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(BIN) $(DIST_DIR)/$(BIN)-linux-amd64
	@echo "Building for darwin-amd64..."
	cargo build --release --target x86_64-apple-darwin
	cp target/x86_64-apple-darwin/release/$(BIN) $(DIST_DIR)/$(BIN)-darwin-amd64
	@echo "Building for windows-amd64..."
	cargo build --release --target x86_64-pc-windows-msvc
	cp target/x86_64-pc-windows-msvc/release/$(BIN).exe $(DIST_DIR)/$(BIN)-windows-amd64.exe
	$(MAKE) zip-all

zip-all:
	cd $(DIST_DIR) && zip $(BIN)-linux-amd64.zip $(BIN)-linux-amd64
	cd $(DIST_DIR) && zip $(BIN)-darwin-amd64.zip $(BIN)-darwin-amd64
	cd $(DIST_DIR) && zip $(BIN)-windows-amd64.zip $(BIN)-windows-amd64.exe

clean:
	rm -rf $(DIST_DIR)
	mkdir -p $(DIST_DIR)

build-local:
	cargo build --release
	cp target/release/$(BIN) $(HOME)/.cargo/bin/$(BIN)
```

### `make.ps1` (Windows)

Same structure as Go version but using `cargo build --release --target ...` and `Copy-Item`.

---

## Verification

After completing all steps:

1. **`cargo build`** — must compile with zero warnings.
2. **`cargo test`** — all unit and integration tests pass.
3. **Manual test matrix**:
   - `gref foo` in a project dir → search-only mode, results displayed.
   - `gref foo bar` → replace mode, select with Space/a, confirm with Enter.
   - `gref -i Foo bar` → case-insensitive search.
   - `gref -e ".git,*.log,media/" foo bar src/` → exclusion works.
   - `gref --help` → prints help and exits.
   - Arrow keys, j/k, h/l, Home/End → navigation works.
   - Test on both Windows and a Unix system (WSL is fine).
4. **`cargo clippy`** — no warnings.
5. **`cargo build --release`** — produces an optimized stripped binary.

## Decisions

- **1 crate only (`regex`)**: all terminal handling is manual ANSI + FFI. This maximizes control and minimizes supply-chain risk.
- **No `crossterm`/`ratatui`**: replaced by `term.rs` with ~200 lines of platform-specific code.
- **No `clap`**: CLI parsing is ~60 lines of manual `std::env::args()` processing.
- **No `walkdir`**: recursive directory walking is ~30 lines with `std::fs::read_dir`.
- **No `rayon`**: parallelism via `std::thread` + `std::sync::mpsc` channels.
- **Atomic file replacement**: temp file in same directory + `std::fs::rename`, matching Go's approach.
- **Display highlighting uses escaped pattern** (literal match), matching Go's `regexp.QuoteMeta` behavior — this is intentional for consistency with the original.
