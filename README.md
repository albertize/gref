# GREF

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange)](https://www.rust-lang.org/)

A fast, interactive search and replace tool for your terminal — built for speed. No TUI framework — raw ANSI escapes and platform FFI for maximum performance and minimal binary size.

---

![GREF terminal UI](/media/gref_base.png)

![GREF Vim integration](/media/gref_vim.png)

## Features

-  **Buffered search engine**: literal-by-default search on top of whole-buffer regex matching with SIMD-accelerated literal pre-filtering (`memchr::memmem`)
-  **Interactive TUI** for previewing and selecting replacements
-  **Smart selection**: choose lines to replace, bulk select/deselect
-  **Atomic file writes** for safe replacements (temp file + rename)
-  **Flicker-free rendering** via cursor-home + line-level clearing
-  **Tiny binary**: only `regex` + `memchr` (transitive), release builds with LTO and strip
-  **UTF-8 safe**: proper char-boundary handling for multi-byte content
-  **Project level filtering**: `.gitignore`, `.ignore`, `.grefignore` support with hierarchical merging
-  **Hidden file skipping**: dot-prefix (Unix) and `FILE_ATTRIBUTE_HIDDEN` (Windows)
-  **Smart binary detection**: known extensions via lookup, unknown extensions via SIMD null-byte scan on already-loaded buffer

---

## Install

### Install script

```sh
curl -fsSL https://raw.githubusercontent.com/albertize/gref/main/install.sh | sh
```

Install a specific release:

```sh
curl -fsSL https://raw.githubusercontent.com/albertize/gref/main/install.sh | sh -s -- --version v2.2.0
```

Useful options:

```sh
curl -fsSL https://raw.githubusercontent.com/albertize/gref/main/install.sh | sh -s -- --no-vim
curl -fsSL https://raw.githubusercontent.com/albertize/gref/main/install.sh | sh -s -- --vim-pack
curl -fsSL https://raw.githubusercontent.com/albertize/gref/main/install.sh | sh -s -- --prefix /usr/local
```

The installer downloads the matching release asset, verifies it with `SHA256SUMS`, installs `gref`, and installs the Vim runtime by default. Default user-local paths are `~/.local/bin/gref` and `~/.vim/`.

Inspectable install:

```sh
curl -fsSLO https://raw.githubusercontent.com/albertize/gref/main/install.sh
less install.sh
sh install.sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/albertize/gref/main/install.ps1 | iex
```

### Download release archives

Go to [Releases](https://github.com/albertize/gref/releases) and download the archive for your platform:

| OS | amd64 | arm64 |
|---|---|---|
| Linux | `gref-linux-amd64.tar.gz` | `gref-linux-arm64.tar.gz` |
| macOS | `gref-darwin-amd64.tar.gz` | `gref-darwin-arm64.tar.gz` |
| Windows | `gref-windows-amd64.zip` | `gref-windows-arm64.zip` |

Release archives include:

```text
bin/gref
vim/plugin/gref.vim
vim/autoload/gref.vim
install.sh
install.ps1
README.md
LICENSE
```

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
- `-v`, `--version` : Show version information and exit
- `-i`, `--ignore-case` : Ignore case in pattern matching
- `-r`, `--regex` : Treat `<pattern>` as a regular expression (default: literal text)
- `-e`, `--exclude` : Exclude path, file or extension (comma separated, e.g. `.git,*.log,media/`)
- `--hidden` : Include hidden files and directories (default outside Git repo roots)
- `--no-ignore` : Don't respect `.gitignore`, `.ignore`, and `.grefignore` files
- `--root PATH` : Search this file or directory
- `--vim-result FILE` : Write the selected search result for Vim integration

### Arguments

- `<pattern>`: Literal text to search for, unless `--regex` is used
- `[replacement]`: Replacement string (if omitted, search-only mode)
- `[directory]`: Directory to search (default: current directory)

In default literal mode, replacement text is written literally. Capture expansion such as `$1` is available only with `--regex`.

### Examples

```sh
gref foo bar src      # Replace 'foo' with 'bar' in src directory
gref foo              # Search for 'foo' only
gref '1.2.0'          # Search for literal dots, not regex wildcards
gref -r 'foo.*bar'    # Search with a regular expression
gref -i Foo           # Case-insensitive search for 'Foo'
gref --version        # Show version information
gref -e .git,*.log    # Exclude .git folders and .log files
gref --hidden foo     # Include hidden files in search
gref --no-ignore foo  # Ignore .gitignore rules
gref --root src foo    # Search a specific file or directory via option
gref --help           # Show help message
```

### Vim Integration

`gref` ships a minimal Vim runtime integration that uses Vim's built-in popup terminal API. No plugin manager is required.

Install manually:

```sh
mkdir -p ~/.vim/plugin ~/.vim/autoload
cp contrib/vim/plugin/gref.vim ~/.vim/plugin/gref.vim
cp contrib/vim/autoload/gref.vim ~/.vim/autoload/gref.vim
```

Then in Vim:

```vim
:Gref foo          " search current Vim working directory, Enter jumps to result
:Gref foo bar      " replace across current Vim working directory
:GrefBuffer foo    " search only the current file
:GrefBuffer foo bar
:Gref --regex --ignore-case 'foo\s+bar'
:Gref --root src foo
```

Replace commands refuse to run while affected Vim buffers have unsaved changes. After replacements, Vim runs `:checktime` so changed files can be reloaded.

Optional popup styling:

```vim
let g:gref_popup_width_percent = 85
let g:gref_popup_height_percent = 80
let g:gref_popup_title = ''
let g:gref_popup_padding = [0, 0, 0, 0]
let g:gref_popup_border = []
let g:gref_popup_borderchars = ['─', '│', '─', '│', '╭', '╮', '╯', '╰']
let g:gref_default_args = []
let g:gref_open_command = 'edit'
```

---

## Keyboard Controls

| Key | Action |
|---|---|
| `↑`/`↓` or `j`/`k` | Move cursor up/down |
| `←`/`→` or `h`/`l` | Scroll horizontally |
| `Home`/`End` | Scroll to start/end of line |
| `v` | Open current result in `$VISUAL`, `$EDITOR`, or `vim` |
| `Space` | Select/deselect a result for replacement |
| `a` | Select all results |
| `n` | Deselect all results |
| `Enter` | Confirm selected replacements; in Vim search integration, open current result |
| `Esc` | Cancel confirmation |
| `q` / `Ctrl+C` | Exit |

---

## Project Structure

```
src/
  main.rs          CLI entry, pattern compile, search, model init, app::run()
  lib.rs           Public module re-exports (enables integration tests)
  cli.rs           Manual argument parsing (no clap). Flags: -v, -i, -r, -e, --hidden, --no-ignore
  model.rs         SearchResult, AppState, AppMode, Model
  search.rs        Pipelined walk + parallel bytes::Regex search, literal prefilter
  replace.rs       Atomic file replacement via temp file + rename
  term.rs          Raw mode FFI (Windows/Unix), ANSI escapes, Key enum, paint()
  ui.rs            Screen rendering (pure function → String), line truncation
  app.rs           Event loop: render → read_key → dispatch → state update
  exclude.rs       Path exclusion (dir/, *.ext, exact filename)
  filedetect.rs    Text vs binary detection (extension lookup + SIMD null-byte scan)
  gitignore.rs     .gitignore/.ignore/.grefignore parsing, glob→regex, hierarchical merging
  integration.rs   Vim result-file writer for editor integration
tests/
  stress_tests.rs  Edge-case and stress tests across all modules
  vim_runtime_tests.rs  Vimscript runtime integration checks
install.sh         Unix bootstrap/release-archive installer
install.ps1        Windows PowerShell bootstrap/release-archive installer
```

---

## Performance

- **Literal default, regex opt-in**: user patterns are escaped by default; `--regex` feeds raw regex syntax to the engine
- **Whole-buffer regex engine**: feeds entire file buffers to `find_iter()` — lets the regex engine's SIMD/Teddy/Aho-Corasick optimizations skip non-matching regions at hardware speed
- **SIMD literal pre-filtering**: `memchr::memmem::Finder` rejects files that lack a literal substring before engaging the regex engine
- **Pipelined parallel search**: file walker dispatches jobs immediately via channel; worker threads start searching as files are discovered
- **Zero-copy path filtering**: OsStr-based hidden/skip/gitignore checks run before `entry.path()` allocates the full PathBuf
- **Deferred binary detection**: known extensions classified without I/O; unknown extensions checked via SIMD null-byte scan on already-loaded buffer
- **Atomic replacements**: writes to temp file, then renames over original
- **Flicker-free TUI**: single locked `stdout` write per frame — no full-screen clear
- **Minimal footprint**: only `regex` + `memchr`; no TUI framework, async runtime, or allocator

---

## Building & Testing

```sh
cargo build                    # Dev build
cargo build --release          # Release (strip=true, lto=true, opt-level=3)
cargo test                     # Unit, stress/edge-case, and Vim runtime tests
cargo clippy                   # Must pass with 0 warnings
```

## Contributing

Contributions are welcome! Please submit issues or pull requests for bug fixes, features, or improvements.

---

## License

MIT License
