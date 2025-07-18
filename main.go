package main

import (
	"flag"
	"fmt"
	"os"
	"regexp"

	grefcore "github.com/albertize/gref/core"
	tea "github.com/charmbracelet/bubbletea"
)

func main() {

	helpText := `gref - search and replace tool

Usage:
  gref [options] <pattern> [replacement] [directory]

Options:
  -h, --help          Show this help message and exit
  -i, --ignore-case   Ignore case in pattern matching
  -e, --exclude       Exclude path, file or extension (comma separated, e.g. ".git,*.log,media/")

Arguments:
  <pattern>         Regex pattern to search for
  [replacement]     Replacement string (if omitted, only search)
  [directory]       Directory to search (default: current directory)

Examples:
  gref foo bar src      Replace 'foo' with 'bar' in src directory
  gref foo              Search for 'foo' only
  gref -i Foo           Search for 'Foo' (case-insensitive)
  gref --help           Show help message
  gref -e .git,*.log    Exclude .git folders and .log files
  `

	// Use flag package for argument parsing
	importFlag := false
	ignoreCase := false
	excludeStr := ""
	flagSet := flag.NewFlagSet(os.Args[0], flag.ExitOnError)
	flagSet.BoolVar(&ignoreCase, "i", false, "Ignore case in pattern matching")
	flagSet.BoolVar(&ignoreCase, "ignore-case", false, "Ignore case in pattern matching")
	flagSet.BoolVar(&importFlag, "h", false, "Show help message")
	flagSet.BoolVar(&importFlag, "help", false, "Show help message")
	flagSet.StringVar(&excludeStr, "e", "", "Exclude path, file or extension (comma separated)")
	flagSet.StringVar(&excludeStr, "exclude", "", "Exclude path, file or extension (comma separated)")

	// Parse flags
	err := flagSet.Parse(os.Args[1:])
	if err != nil {
		fmt.Println("Error parsing flags:", err)
		os.Exit(1)
	}

	args := flagSet.Args()
	if importFlag {
		fmt.Print(helpText)
		os.Exit(0)
	}

	if len(args) < 1 {
		fmt.Println("Usage: gref [options] <pattern> [replacement] [directory]")
		fmt.Println("Try 'gref --help' for more information.")
		os.Exit(0)
	}

	patternStr := args[0]
	replacementStr := ""
	rootPath := "."

	mode := grefcore.Default
	if len(args) > 1 {
		replacementStr = args[1]
	} else {
		mode = grefcore.SearchOnly
	}
	if len(args) > 2 {
		rootPath = args[2]
	}

	// Compile the regex pattern (case-insensitive if requested)
	var pattern *regexp.Regexp
	if ignoreCase {
		pattern, err = regexp.Compile("(?i)" + patternStr)
	} else {
		pattern, err = regexp.Compile(patternStr)
	}
	if err != nil {
		fmt.Printf("Error compiling regex pattern: %v\n", err)
		os.Exit(1)
	}

	// Parse exclude patterns
	var excludeList []string
	if excludeStr != "" {
		excludeList = grefcore.ParseExcludeList(excludeStr)
	}

	// Perform the initial search
	results, err := grefcore.PerformSearchAdaptive(rootPath, pattern, excludeList)
	if err != nil {
		fmt.Printf("Error during search: %v\n", err)
		os.Exit(1)
	}

	if len(results) == 0 {
		fmt.Println("No results found for the pattern:", patternStr)
		os.Exit(0)
	}

	// Initialize the Bubble Tea model in AltScreen (dedicated buffer)
	p := tea.NewProgram(grefcore.InitModel(results, patternStr, replacementStr, pattern, mode), tea.WithAltScreen())

	// Start the Bubble Tea program
	if _, err := p.Run(); err != nil {
		fmt.Printf("Error running the program: %v\n", err)
		os.Exit(1)
	}

}
