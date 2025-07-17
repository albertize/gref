# GREF

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Go](https://img.shields.io/badge/Go-1.23%2B-blue)](https://golang.org/)

A fast, interactive search and replace tool for your terminal, powered by [Bubble Tea](https://github.com/charmbracelet/bubbletea) and [Lipgloss](https://github.com/charmbracelet/lipgloss).

---
![GREF Demo](/media/GREF-Demo.gif)

## Features

- 🚀 **Fast regex search** across files and directories
- 🖥️ **Interactive TUI** for previewing and selecting replacements
- 🧠 **Smart selection**: choose lines to replace, bulk select/deselect
- 🛡️ **Atomic file writes** for safe replacements
- 🎨 **Customizable styles** and clear error messages
- 🏃 **Efficient for large codebases**

---

## Install


### Go install (standard)
```sh
go install github.com/albertize/gref@latest
```


### Download pre-built binaries from GitHub Releases
Go to [github.com/albertize/gref/releases](https://github.com/albertize/gref/releases) and download the package for your platform (Linux, macOS, Windows) from the "Assets" section.

### Local build via makefile
You can also build binaries locally:

```sh
make build-all
```
You will find the binaries in the `dist/` folder for Linux, macOS, and Windows.

For a quick local build and install:
```sh
make build-local
```
The binary will be copied to `$HOME/go/bin`.

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
- `[replacement]`: Replacement string (if omitted, only search)
- `[directory]`: Directory to search (default: current directory)

### Example

```sh
gref foo bar src      # Replace 'foo' with 'bar' in src directory
gref foo              # Search for 'foo' only
gref -i Foo           # Case-insensitive search for 'Foo'
gref --help           # Show help message
gref -e .git,*.log    # Exclude .git folders and .log files
```

### Options

- `-h`, `--help` : Show help message and exit
- `-i`, `--ignore-case` : Ignore case in pattern matching

### Arguments

- `<pattern>`: Regex pattern to search for
- `[replacement]`: Replacement string (if omitted, only search)
- `[directory]`: Directory to search (default: current directory)

### Example

```sh
gref foo bar src      # Replace 'foo' with 'bar' in src directory
gref foo              # Search for 'foo' only
gref -i Foo           # Case-insensitive search for 'Foo'
gref --help           # Show help message
```

---

## Keyboard Controls

- `↑`/`↓`/`j`/`k`: Move cursor up/down
- `←`/`→`/`h`/`l`: Scroll horizontally
- `Home`/`End`: Scroll to start/end of line
- `Space`: Select/deselect a result for replacement
- `a`: Select all results
- `n`: Deselect all results
- `Enter`: Confirm selected replacements
- `Esc`: Cancel confirmation
- `q`/`Ctrl+c`: Exit

---

## Project Structure

- **main.go**: CLI entry, argument parsing, help, and UI launch
- **model.go**: TUI state, rendering, and event handling
- **search.go**: Efficient regex search across files, exclusion logic
- **replace.go**: Safe, grouped replacements in files
- **test/replace_test.go**: Extensive edge case tests for replace (Windows/Unix line endings, empty files, binary/null bytes, permission errors, overlapping matches, invalid regex, etc.)
- **test/search_test.go**: Tests for exclude logic and pattern parsing

---

## Performance

GREF is designed for speed and efficiency:

- **Optimized Search**: Buffered reading and byte-level processing for large files
- **Parallel File Traversal**: Uses Go concurrency for fast directory scanning
- **Atomic Replacements**: Writes changes to temp files before replacing originals
- **Minimal UI Overhead**: Responsive TUI adapts to terminal size
- **Selective Processing**: Only selected lines/files are modified

---

## Related Projects

- [Bubble Tea](https://github.com/charmbracelet/bubbletea): TUI framework
- [Lipgloss](https://github.com/charmbracelet/lipgloss): Terminal style toolkit

---

## Contributing

Contributions are welcome! Please submit issues or pull requests for bug fixes, features, or improvements.

---

## License

MIT License