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
	clearScreenANSI()

	rootPath := os.Args[1]
	patternStr := os.Args[2]
	replacementStr := ""
	if len(os.Args) > 3 {
		replacementStr = os.Args[3]
	}

	// Compila il pattern regex
	pattern, err := regexp.Compile(patternStr)
	if err != nil {
		clearScreenANSI()
		fmt.Printf("Error compiling regex pattern: %v\n", err)
		os.Exit(1)
	}

	// Esegui la ricerca iniziale
	results, err := performSearch(rootPath, pattern)
	if err != nil {
		clearScreenANSI()
		fmt.Printf("Error during search: %v\n", err)
		os.Exit(1)
	}

	if len(results) == 0 {
		clearScreenANSI()
		fmt.Println("No results found for the pattern:", patternStr)
		os.Exit(0)
	}

	// Inizializza il modello Bubble Tea
	p := tea.NewProgram(initialModel(results, patternStr, replacementStr))

	// Avvia il programma Bubble Tea
	if _, err := p.Run(); err != nil {
		clearScreenANSI()
		fmt.Printf("Error running the program: %v\n", err)
		os.Exit(1)
	}
}
