# GREF (Global Replace and Find)

gref is a terminal-based search and replace tool for text files, featuring an interactive TUI (Text User Interface) built with Bubble Tea and Lipgloss. It allows users to search for patterns, preview results, select lines for replacement, and perform replacements across files with robust controls and clear feedback.

## Features

- **Search and Replace**: Search for regex patterns in files and optionally replace them with a given string.
- **Interactive UI**: Browse results, select/deselect lines, confirm replacements, and view status/errors in a modern terminal interface.
- **Horizontal and Vertical Scrolling**: Navigate long lines and large result sets with keyboard controls.
- **Bulk Selection**: Select/deselect all results for replacement.
- **Preview**: See highlighted matches and replacement previews before confirming changes.
- **Robust Error Handling**: Handles file access errors, invalid patterns, and displays clear error messages.
- **Modes**: Supports search-only and search-and-replace modes.

## Usage

```
gref <pattern> [replacement] [directory]
```

### Options

- `-h`, `--help` : Show help message and exit

### Arguments

- `<pattern>`: Regex pattern to search for
- `[replacement]`: Replacement string (if omitted, only search)
- `[directory]`: Directory to search (default: current directory)

### Examples

- `gref foo bar src`      Replace 'foo' with 'bar' in src directory
- `gref foo`              Search for 'foo' only
- `gref --help`           Show help message

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

## Implementation Details

### Main Components

- **main.go**: Entry point, argument parsing, help message, and Bubble Tea program initialization.
- **model.go**: TUI state management, event handling, rendering logic, and UI controls.
- **search.go**: File system traversal and regex search, returning results with file, line, and match info.
- **replace.go**: Performs replacements in selected files/lines, writes changes atomically.
- **test/test.go**: Example/test file for API calls and constants, with translated comments and dummy data.

### Data Structures

- `SearchResult`: Holds file path, line number, line text, and matched text for each result.
- `model`: Maintains UI state, results, selection, cursor, scrolling, error info, and adapts to terminal width (`screenWidth`).

### Error Handling

- All file operations and regex compilation are wrapped with error checks and clear messages.

### Customization

- Colors and styles are defined for highlights, replacements, selections, and help text.

## Contributing

Contributions are welcome! Please submit issues or pull requests for bug fixes, features, or improvements.

## License

MIT License