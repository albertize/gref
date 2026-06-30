# GREF

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange)](https://www.rust-lang.org/)

**Interactive project-wide search and replace for the terminal and Vim.**

GREF helps you replace text across a project without doing it blindly. It searches your files, shows every match in a fast terminal UI, lets you choose exactly which results to replace, and writes changes safely.

Use it when a global replacement feels risky, but editing matches one by one is too slow.

---

![GREF terminal UI](/media/gref_base.png)

![GREF Vim integration](/media/gref_vim.png)

## Why GREF?

Most search-and-replace workflows are either too automatic or too manual.

* `sed` and scripts are fast, but easy to misuse.
* Editor-based replacements are convenient, but often tied to one editor session.
* Search tools show matches, but usually do not give you a safe interactive replacement flow.

GREF sits in the middle: fast enough for project-wide changes, but interactive enough to keep you in control.

It is useful for:

* renaming identifiers across a codebase
* updating configuration keys
* changing repeated documentation text
* reviewing risky replacements before writing files
* doing project-wide replacements from Vim
* searching and replacing while respecting ignore files

## Quick Start

Replace `foo` with `bar` inside `src`:

```sh
gref foo bar src
```

Search only, without replacing:

```sh
gref TODO
```

Search for literal text by default:

```sh
gref '1.2.0'
```

Use a regular expression:

```sh
gref --regex 'foo\s+bar' replacement .
```

Search case-insensitively:

```sh
gref --ignore-case Foo
```

Exclude paths, files, or extensions:

```sh
gref -e .git,*.log,media/ foo
```

## Features

* **Interactive replacement preview** — review matches before changing files
* **Selective replacements** — choose individual results or bulk select/deselect
* **Literal search by default** — search text as text, with regex available through `--regex`
* **Safe file updates** — replacements are written atomically using a temporary file and rename
* **Project-aware filtering** — respects `.gitignore`, `.ignore`, and `.grefignore`
* **Hidden file handling** — skips hidden files by default, with an option to include them
* **Binary file detection** — avoids replacing inside binary files
* **UTF-8 safe replacements** — handles multi-byte text correctly
* **Vim integration** — run GREF from Vim and jump back to selected results
* **Small and fast** — written in Rust with minimal dependencies and no TUI framework

## Install

### Install Script

Unix, Linux, and macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/albertize/gref/master/install.sh | sh
```

Install a specific release:

```sh
curl -fsSL https://raw.githubusercontent.com/albertize/gref/master/install.sh | sh -s -- --version vX.Y.Z
```

Useful options:

```sh
curl -fsSL https://raw.githubusercontent.com/albertize/gref/master/install.sh | sh -s -- --no-vim
curl -fsSL https://raw.githubusercontent.com/albertize/gref/master/install.sh | sh -s -- --vim-pack
curl -fsSL https://raw.githubusercontent.com/albertize/gref/master/install.sh | sh -s -- --prefix /usr/local
```

The installer downloads the matching release asset, verifies it with `SHA256SUMS`, installs `gref`, and installs the Vim runtime by default.

Default user-local paths:

```text
~/.local/bin/gref
~/.vim/
```

Inspectable install:

```sh
curl -fsSLO https://raw.githubusercontent.com/albertize/gref/master/install.sh
less install.sh
sh install.sh
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/albertize/gref/master/install.ps1 | iex
```

### Download Release Archives

Go to [Releases](https://github.com/albertize/gref/releases) and download the archive for your platform.

| OS      | amd64                      | arm64                      |
| ------- | -------------------------- | -------------------------- |
| Linux   | `gref-linux-amd64.tar.gz`  | `gref-linux-arm64.tar.gz`  |
| macOS   | `gref-darwin-amd64.tar.gz` | `gref-darwin-arm64.tar.gz` |
| Windows | `gref-windows-amd64.zip`   | `gref-windows-arm64.zip`   |

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

### Build From Source

```sh
cargo build --release
cargo install --path .
```

## Usage

```sh
gref [options] <pattern> [replacement] [directory]
```

### Arguments

* `<pattern>` — literal text to search for, unless `--regex` is used
* `[replacement]` — replacement text; if omitted, GREF runs in search-only mode
* `[directory]` — directory to search; defaults to the current directory

In default literal mode, replacement text is written literally. Capture expansion such as `$1` is available only with `--regex`.

### Options

| Option                | Description                                                     |
| --------------------- | --------------------------------------------------------------- |
| `-h`, `--help`        | Show help message and exit                                      |
| `-v`, `--version`     | Show version information and exit                               |
| `-i`, `--ignore-case` | Ignore case in pattern matching                                 |
| `-r`, `--regex`       | Treat `<pattern>` as a regular expression                       |
| `-e`, `--exclude`     | Exclude paths, files, or extensions, comma separated            |
| `--hidden`            | Include hidden files and directories                            |
| `--no-ignore`         | Do not respect `.gitignore`, `.ignore`, and `.grefignore` files |
| `--root PATH`         | Search this file or directory                                   |
| `--vim-result FILE`   | Write the selected search result for Vim integration            |

### Examples

```sh
gref foo bar src
```

Replace `foo` with `bar` in the `src` directory.

```sh
gref foo
```

Search for `foo` without replacing anything.

```sh
gref '1.2.0'
```

Search for literal dots, not regex wildcards.

```sh
gref --regex 'foo.*bar'
```

Search with a regular expression.

```sh
gref --ignore-case Foo
```

Search for `Foo` case-insensitively.

```sh
gref -e .git,*.log,media/ foo
```

Exclude `.git` folders, `.log` files, and the `media/` directory.

```sh
gref --hidden foo
```

Include hidden files in the search.

```sh
gref --no-ignore foo
```

Ignore `.gitignore`, `.ignore`, and `.grefignore` rules.

```sh
gref --root src foo
```

Search a specific file or directory using an option.

## Keyboard Controls

| Key                    | Action                                                |
| ---------------------- | ----------------------------------------------------- |
| `↑` / `↓` or `j` / `k` | Move cursor up or down                                |
| `←` / `→` or `h` / `l` | Scroll horizontally                                   |
| `Home` / `End`         | Scroll to start or end of line                        |
| `Space`                | Select or deselect a result for replacement           |
| `a`                    | Select all results                                    |
| `n`                    | Deselect all results                                  |
| `Enter`                | Confirm selected replacements                         |
| `v`                    | Open current result in `$VISUAL`, `$EDITOR`, or `vim` |
| `Esc`                  | Cancel confirmation                                   |
| `q` / `Ctrl+C`         | Exit                                                  |

In Vim search integration, `Enter` opens the selected result.

## Vim Integration

GREF ships a minimal Vim runtime integration that uses Vim's built-in popup terminal API. No plugin manager is required.

If you install GREF with the default install script, the Vim runtime is installed automatically.

### Manual Vim Install

```sh
mkdir -p ~/.vim/plugin ~/.vim/autoload
cp contrib/vim/plugin/gref.vim ~/.vim/plugin/gref.vim
cp contrib/vim/autoload/gref.vim ~/.vim/autoload/gref.vim
```

### Vim Commands

```vim
:Gref foo
```

Search the current Vim working directory. Press `Enter` on a result to jump to it.

```vim
:Gref foo bar
```

Replace across the current Vim working directory.

```vim
:GrefBuffer foo
```

Search only the current file.

```vim
:GrefBuffer foo bar
```

Replace only in the current file.

```vim
:Gref --regex --ignore-case 'foo\s+bar'
```

Use regex and case-insensitive search.

```vim
:Gref --root src foo
```

Search a specific root from Vim.

Replace commands refuse to run while affected Vim buffers have unsaved changes. After replacements, Vim runs `:checktime` so changed files can be reloaded.

### Optional Vim Popup Styling

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

## Safety

GREF is designed to make project-wide replacement safer.

* It previews matches before writing changes.
* It lets you select exactly which results to replace.
* It writes files atomically using a temporary file and rename.
* It respects ignore files by default.
* It skips hidden files unless requested.
* It avoids binary files.
* In Vim, it refuses replacements when affected buffers have unsaved changes.

GREF is still a file-modifying tool. For important changes, use version control and review the diff before committing.

```sh
git diff
```

## Ignore Rules and Filtering

By default, GREF respects:

```text
.gitignore
.ignore
.grefignore
```

Ignore rules are merged hierarchically as GREF walks the project.

You can disable ignore handling:

```sh
gref --no-ignore foo
```

You can include hidden files:

```sh
gref --hidden foo
```

You can exclude paths, files, or extensions manually:

```sh
gref -e .git,*.log,media/ foo
```

Examples of exclusions:

```text
.git
*.log
media/
target/
node_modules/
```

## Literal Search and Regex Mode

GREF searches literal text by default.

That means this command searches for the exact string `1.2.0`:

```sh
gref '1.2.0'
```

The dots are treated as dots, not as regex wildcards.

To use regular expressions, pass `--regex`:

```sh
gref --regex 'v([0-9]+)\.([0-9]+)\.([0-9]+)' 'v$1.$2.x'
```

Capture expansion such as `$1` is available only in regex mode.

## Performance Notes

GREF is built for speed without relying on a heavy terminal UI framework.

Technical highlights:

* **Buffered search engine** — searches loaded file buffers efficiently
* **Literal pre-filtering** — uses `memchr::memmem` to quickly reject files that cannot match
* **Whole-buffer regex search** — feeds complete buffers to the regex engine
* **Pipelined parallel search** — starts searching files as they are discovered
* **Zero-copy path filtering** — performs hidden, skip, and ignore checks before unnecessary path allocation
* **Deferred binary detection** — classifies known extensions without I/O and scans unknown buffers for null bytes
* **Flicker-free rendering** — uses cursor-home and line-level clearing instead of full-screen clears
* **Minimal dependencies** — only `regex` and `memchr` as core dependencies
* **Small release builds** — optimized with LTO and stripping

## Building and Testing

```sh
cargo build
cargo build --release
cargo test
cargo clippy
```

Release builds use:

```text
strip = true
lto = true
opt-level = 3
```

Tests include unit, stress, edge-case, and Vim runtime checks.

## Development Notes

Main source layout:

```text
src/
  main.rs          CLI entry point
  cli.rs           Manual argument parsing
  model.rs         Search results and app state
  search.rs        File walking and search pipeline
  replace.rs       Atomic file replacement
  term.rs          Raw terminal handling
  ui.rs            Rendering
  app.rs           Event loop
  exclude.rs       Path exclusion
  filedetect.rs    Text and binary detection
  gitignore.rs     Ignore-file parsing
  integration.rs   Vim result-file writer

tests/
  stress_tests.rs
  vim_runtime_tests.rs
```

## Contributing

Contributions are welcome.

Good first areas to help with:

* bug reports with reproducible examples
* platform-specific testing
* Vim integration improvements
* documentation improvements
* performance testing on large repositories
* packaging for more distribution channels

Please open an issue or pull request for fixes, features, or improvements.

## License

MIT License
