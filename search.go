package main

import (
	"bufio"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
)

// SearchResult holds information about a found match
type SearchResult struct {
	FilePath  string
	LineNum   int
	LineText  string
	MatchText string // The exact text that matched the pattern
}

// performSearch walks through the directory and finds all matches
func performSearch(rootPath string, pattern *regexp.Regexp) ([]SearchResult, error) {
	results := []SearchResult{}

	err := filepath.WalkDir(rootPath, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			// Error accessing path; log if needed, but continue walking
			// fmt.Printf("Error accessing path %s: %v\n", path, err)
			return nil // Continue walking even if one file fails
		}
		if d.IsDir() {
			return nil // Skip directories
		}

		file, err := os.Open(path)
		if err != nil {
			// Could not open file (e.g., permissions); skip
			return nil
		}
		defer file.Close()

		scanner := bufio.NewScanner(file)
		lineNum := 0
		for scanner.Scan() {
			lineNum++
			line := scanner.Text()
			if pattern.MatchString(line) {
				// Store the matched string for reporting
				match := pattern.FindString(line)

				results = append(results, SearchResult{
					FilePath:  path,
					LineNum:   lineNum,
					LineText:  line,
					MatchText: match,
				})
			}
		}
		if err := scanner.Err(); err != nil {
			// Error reading file; log if needed
			// fmt.Printf("Error reading file %s: %v\n", path, err)
		}
		return nil
	})

	if err != nil {
		return nil, err
	}
	return results, nil
}
