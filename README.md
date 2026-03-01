# GREF

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange)](https://www.rust-lang.org/)

A fast, interactive search and replace tool for your terminal — Zero dependencies beyond `regex`. No TUI framework — raw ANSI escapes and platform FFI for maximum performance and minimal binary size.

---
![GREF Demo](/media/DEMO-GREF.gif)

## Features

- 🚀 **Fast parallel regex search** across files and directories
- 🖥️ **Interactive TUI** for previewing and selecting replacements
- 🧠 **Smart selection**: choose lines to replace, bulk select/deselect
- 🛡️ **Atomic file writes** for safe replacements (temp file + rename)
- 🎨 **Flicker-free rendering** via cursor-home + line-level clearing
- 🏃 **Tiny binary**: single dependency (`regex`), release builds with LTO and `opt-level="z"`
- 🔤 **UTF-8 safe**: proper char-boundary handling for multi-byte content

---

## Install

### Download pre-built binaries

Go to [Releases](https://github.com/albertize/gref/releases) and download the binary for your platform:

| OS | amd64 | arm64 |
|---|---|---|
| Linux | `gref-linux-amd64` | `gref-linux-arm64` |
| macOS | `gref-darwin-amd64` | `gref-darwin-arm64` |
| Windows | `gref-windows-amd64.exe` | `gref-windows-arm64.exe` |

### Build from source

```sh
cargo install --path .
```

### Build and install locally

```sh
cargo build --release
cargo install --path .
```

---

## Usage

```sh
gref [options] <pattern> [replacement] [directory]
```

### Options

- `-h`, `--help` : Show help message and exit
- `-i`, `--ignore-case` : Ignore case in pattern matching
- `-e`, `--exclude` : Exclude path, file or extension (comma separated, e.g. `.git,*.log,media/`)

### Arguments

- `<pattern>`: Regex pattern to search for
- `[replacement]`: Replacement string (if omitted, search-only mode)
- `[directory]`: Directory to search (default: current directory)

### Examples

```sh
gref foo bar src      # Replace 'foo' with 'bar' in src directory
gref foo              # Search for 'foo' only
gref -i Foo           # Case-insensitive search for 'Foo'
gref -e .git,*.log    # Exclude .git folders and .log files
gref --help           # Show help message
```

---

## Keyboard Controls

| Key | Action |
|---|---|
| `↑`/`↓` or `j`/`k` | Move cursor up/down |
| `←`/`→` or `h`/`l` | Scroll horizontally |
| `Home`/`End` | Scroll to start/end of line |
| `Space` | Select/deselect a result for replacement |
| `a` | Select all results |
| `n` | Deselect all results |
| `Enter` | Confirm selected replacements |
| `Esc` | Cancel confirmation |
| `q` / `Ctrl+C` | Exit |

---

## Project Structure

```
src/
  main.rs          CLI entry, regex compile, search, model init, app::run()
  lib.rs           Public module re-exports (enables integration tests)
  cli.rs           Manual argument parsing (no clap)
  model.rs         SearchResult, AppState, AppMode, Model
  search.rs        Parallel regex search with thread pool + mpsc channels
  replace.rs       Atomic file replacement via temp file + rename
  term.rs          Raw mode FFI (Windows/Unix), ANSI escapes, Key enum, paint()
  ui.rs            Screen rendering (pure function → String)
  app.rs           Event loop: render → read_key → dispatch → state update
  exclude.rs       Path exclusion (dir/, *.ext, exact filename)
  filedetect.rs    Text vs binary detection (extension + content probe)
tests/
  stress_tests.rs  87 edge-case and stress tests across all modules
```

---

## Performance

- **Parallel file traversal**: Thread pool with work-stealing via `Arc<Mutex<Receiver<PathBuf>>>`
- **Adaptive search**: Literal prefix pre-filtering before regex matching on large files
- **Buffered I/O**: `BufReader` with 128 KB buffer for large file scanning
- **Atomic replacements**: Writes to temp file, then renames over original
- **Flicker-free TUI**: Single locked `stdout` write per frame — no full-screen clear
- **Minimal footprint**: Only `regex` crate; no runtime allocator, TUI framework, or async runtime

---

## Building & Testing

```sh
cargo build                    # Dev build
cargo build --release          # Release (strip=true, lto=true, opt-level="z")
cargo test                     # 25 unit + 87 stress/edge-case tests
cargo clippy                   # Must pass with 0 warnings
```

## Contributing

Contributions are welcome! Please submit issues or pull requests for bug fixes, features, or improvements.

---

## License

MIT License
