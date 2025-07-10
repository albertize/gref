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
			// Log the error but continue walking
			// fmt.Printf("Errore durante l'accesso al percorso %s: %v\n", path, err)
			return nil // Don't stop the walk for individual errors
		}
		if d.IsDir() {
			return nil // Skip directories
		}

		file, err := os.Open(path)
		if err != nil {
			// Skip files that cannot be opened (e.g., permissions)
			return nil
		}
		defer file.Close()

		scanner := bufio.NewScanner(file)
		lineNum := 0
		for scanner.Scan() {
			lineNum++
			line := scanner.Text()
			if pattern.MatchString(line) {
				// Find the matched string to store it
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
			// fmt.Printf("Errore nella lettura del file %s: %v\n", path, err)
		}
		return nil
	})

	if err != nil {
		return nil, err
	}
	return results, nil
}
