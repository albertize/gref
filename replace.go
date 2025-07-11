package main

import (
	"fmt"
	"os"
	"regexp"
	"strings"
)

// performReplacements executes the actual file modifications
func performReplacements(allResults []SearchResult, selected map[int]struct{}, patternStr, replacementStr string) error {
	// Group results by file path to avoid reading/writing the same file multiple times
	filesToProcess := make(map[string][]SearchResult)
	for i, res := range allResults {
		if _, ok := selected[i]; ok {
			filesToProcess[res.FilePath] = append(filesToProcess[res.FilePath], res)
		}
	}

	// Compile the pattern once for replacement
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

// replaceInFile reads a file, performs replacements, and writes it back
func replaceInFile(filePath string, resultsInFile []SearchResult, pattern *regexp.Regexp, replacementStr string) error {
	// Read the entire file content
	contentBytes, err := os.ReadFile(filePath)
	if err != nil {
		return fmt.Errorf("unable to read file: %w", err)
	}
	content := string(contentBytes)

	// Split content into lines
	lines := strings.Split(content, "\n")

	// Create a map for quick lookup of lines to be replaced
	linesToReplace := make(map[int]struct{})
	for _, res := range resultsInFile {
		// Use line number - 1 because slice indices are 0-based
		linesToReplace[res.LineNum-1] = struct{}{}
	}

	// Iterate through lines and replace only those that were selected
	// Note: This approach replaces ALL occurrences of the pattern on the selected lines.
	// If the requirement was to replace only the *specific* matched instance,
	// the logic would be significantly more complex (e.g., tracking character offsets).
	// Given "minimal", replacing all on a selected line is a reasonable compromise.
	for i := range lines {
		if _, ok := linesToReplace[i]; ok {
			lines[i] = pattern.ReplaceAllString(lines[i], replacementStr)
		}
	}

	// Join lines back and write to a temporary file
	newContent := strings.Join(lines, "\n")
	tempFile := filePath + ".tmp"
	err = os.WriteFile(tempFile, []byte(newContent), 0644) // Use original permissions
	if err != nil {
		return fmt.Errorf("unable to write temporary file: %w", err)
	}

	// Replace the original file with the temporary one
	err = os.Rename(tempFile, filePath)
	if err != nil {
		return fmt.Errorf("unable to rename temporary file: %w", err)
	}

	return nil
}
