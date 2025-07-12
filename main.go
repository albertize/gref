package main

import (
	"fmt"
	"os"
	"regexp"

	tea "github.com/charmbracelet/bubbletea"
)

func main() {
	helpText := `gref - search and replace tool

Usage:
  gref <pattern> [replacement] [directory]

Options:
  -h, --help        Show this help message and exit

Arguments:
  <pattern>         Regex pattern to search for
  [replacement]     Replacement string (if omitted, only search)
  [directory]       Directory to search (default: current directory)

Examples:
  gref foo bar src      Replace 'foo' with 'bar' in src directory
  gref foo              Search for 'foo' only
  gref --help           Show this help message
  `

	if len(os.Args) < 2 {
		fmt.Println("Usage: gref <pattern> [replacement] [directory]")
		fmt.Println("Try 'gref --help, -h' for more information.")
		os.Exit(0)
	}

	if os.Args[1] == "--help" || os.Args[1] == "-h" {
		fmt.Print(helpText)
		os.Exit(0)
	}

	patternStr := os.Args[1]
	replacementStr := ""
	rootPath := "."
	mode := Default
	if len(os.Args) > 2 {
		replacementStr = os.Args[2]
	} else {
		mode = SearchOnly
	}

	if len(os.Args) > 3 {
		rootPath = os.Args[3]
	}

	// Compile the regex pattern
	pattern, err := regexp.Compile(patternStr)
	if err != nil {
		fmt.Printf("Error compiling regex pattern: %v\n", err)
		os.Exit(1)
	}

	// Perform the initial search
	results, err := performSearch(rootPath, pattern)
	if err != nil {
		fmt.Printf("Error during search: %v\n", err)
		os.Exit(1)
	}

	if len(results) == 0 {
		fmt.Println("No results found for the pattern:", patternStr)
		os.Exit(0)
	}

	// Initialize the Bubble Tea model in AltScreen (dedicated buffer)
	p := tea.NewProgram(initialModel(results, patternStr, replacementStr, mode), tea.WithAltScreen())

	// Start the Bubble Tea program
	if _, err := p.Run(); err != nil {
		fmt.Printf("Error running the program: %v\n", err)
		os.Exit(1)
	}
}
