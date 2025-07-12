package main

import (
	"fmt"
	"os"
	"regexp"

	tea "github.com/charmbracelet/bubbletea"
)

func main() {
	if len(os.Args) < 3 {
		fmt.Println("Example: go run . <directory> <pattern> [replacement]")
		fmt.Println("         (Use '.' for the current folder)")
		os.Exit(1)
	}

	rootPath := os.Args[1]
	patternStr := os.Args[2]
	replacementStr := ""
	if len(os.Args) > 3 {
		replacementStr = os.Args[3]
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
	p := tea.NewProgram(initialModel(results, patternStr, replacementStr), tea.WithAltScreen())

	// Start the Bubble Tea program
	if _, err := p.Run(); err != nil {
		fmt.Printf("Error running the program: %v\n", err)
		os.Exit(1)
	}
}
