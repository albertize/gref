package grefcore

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"regexp"
)

// PerformReplacements coordinates the replacement process across multiple files.
// It groups results by file path to ensure each file is opened and written only once.
func PerformReplacements(allResults []SearchResult, selected map[int]struct{}, pattern *regexp.Regexp, replacementStr string) error {
	// Group selected results by file path
	filesToProcess := make(map[string][]SearchResult)
	for i, res := range allResults {
		if _, ok := selected[i]; ok {
			filesToProcess[res.FilePath] = append(filesToProcess[res.FilePath], res)
		}
	}

	// Process files. In a high-performance scenario, this could be parallelized,
	// but we'll stick to sequential to avoid disk thrashing.
	for filePath, resultsInFile := range filesToProcess {
		if err := ReplaceInFile(filePath, resultsInFile, pattern, replacementStr); err != nil {
			return fmt.Errorf("replacement failed for %s: %w", filePath, err)
		}
	}
	return nil
}

// ReplaceInFile streams the file line-by-line to a temporary file, applying replacements.
// This approach is memory-efficient and works on files of any size.
func ReplaceInFile(filePath string, resultsInFile []SearchResult, pattern *regexp.Regexp, replacementStr string) (err error) {
	// 1. Prepare a map of line numbers that need modification (1-based)
	linesToReplace := make(map[int]struct{})
	for _, res := range resultsInFile {
		linesToReplace[res.LineNum] = struct{}{}
	}

	// 2. Open the source file
	src, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("failed to open source: %w", err)
	}
	defer src.Close()

	// 3. Create a temporary file in the same directory (ensures os.Rename works across partitions)
	tmpFile, err := os.CreateTemp(filepath.Dir(filePath), "gref_tmp_*")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %w", err)
	}

	// Ensure cleanup in case of failure
	tmpPath := tmpFile.Name()
	defer func() {
		if err != nil {
			tmpFile.Close()
			os.Remove(tmpPath)
		}
	}()

	// 4. Stream and replace
	reader := bufio.NewReader(src)
	writer := bufio.NewWriter(tmpFile)

	lineNum := 0
	for {
		lineNum++
		// ReadString('\n') preserves the original delimiter (\n or \r\n)
		line, readErr := reader.ReadString('\n')
		if readErr != nil && readErr != io.EOF {
			return fmt.Errorf("error reading line %d: %w", lineNum, readErr)
		}

		processedLine := line
		if _, ok := linesToReplace[lineNum]; ok {
			processedLine = pattern.ReplaceAllString(line, replacementStr)
		}

		if _, writeErr := writer.WriteString(processedLine); writeErr != nil {
			return fmt.Errorf("error writing to temp file: %w", writeErr)
		}

		if readErr == io.EOF {
			break
		}
	}

	// 5. Flush and close files before renaming
	if err = writer.Flush(); err != nil {
		return fmt.Errorf("failed to flush buffer: %w", err)
	}
	tmpFile.Close()
	src.Close()

	// 6. Atomic swap: replace the original file with the updated one
	if err = os.Rename(tmpPath, filePath); err != nil {
		return fmt.Errorf("failed to finalize replacement: %w", err)
	}

	return nil
}
