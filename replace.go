package main

import (
	"fmt"
	"os"
	"regexp"
	"strings"
)

// performReplacements modifies files by applying replacements to selected results
func performReplacements(allResults []SearchResult, selected map[int]struct{}, patternStr, replacementStr string) error {
	// Group results by file path to minimize file reads and writes
	filesToProcess := make(map[string][]SearchResult)
	for i, res := range allResults {
		if _, ok := selected[i]; ok {
			filesToProcess[res.FilePath] = append(filesToProcess[res.FilePath], res)
		}
	}

	// Compile the regex pattern once for efficient replacement
	pattern, err := regexp.Compile(patternStr)
	if err != nil {
		return fmt.Errorf("error recompiling pattern for replacement: %w", err)
	}

	for filePath, resultsInFile := range filesToProcess {
		err := replaceInFile(filePath, resultsInFile, pattern, replacementStr)
		if err != nil {
			return fmt.Errorf("error during replacement in file %s: %w", filePath, err)
		}
	}
	return nil
}

// replaceInFile reads a file, applies replacements to selected lines, and writes the result
func replaceInFile(filePath string, resultsInFile []SearchResult, pattern *regexp.Regexp, replacementStr string) error {
	// Read the full file content into memory
	contentBytes, err := os.ReadFile(filePath)
	if err != nil {
		return fmt.Errorf("unable to read file: %w", err)
	}
	content := string(contentBytes)

	// Split the file content into individual lines
	lines := strings.Split(content, "\n")

	// Build a map for fast lookup of which lines need replacement
	linesToReplace := make(map[int]struct{})
	for _, res := range resultsInFile {
		// Convert line numbers to zero-based indices for slices
		linesToReplace[res.LineNum-1] = struct{}{}
	}

	// For each line, replace only if it was selected
	// Note: This replaces ALL occurrences of the pattern on selected lines.
	// To replace only specific matches, more complex logic (e.g., tracking offsets) would be needed.
	// For simplicity, replacing all matches on a selected line is a practical approach.
	for i := range lines {
		if _, ok := linesToReplace[i]; ok {
			lines[i] = pattern.ReplaceAllString(lines[i], replacementStr)
		}
	}

	// Join the lines and write the result to a temporary file
	newContent := strings.Join(lines, "\n")
	tempFile := filePath + ".tmp"
	err = os.WriteFile(tempFile, []byte(newContent), 0644) // Use default permissions
	if err != nil {
		return fmt.Errorf("unable to write temporary file: %w", err)
	}

	// Atomically replace the original file with the new file
	err = os.Rename(tempFile, filePath)
	if err != nil {
		return fmt.Errorf("unable to rename temporary file: %w", err)
	}

	return nil
}
