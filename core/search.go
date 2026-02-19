package grefcore

import (
	"bufio"
	"bytes"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"sync"
)

// Global maps for file classification to avoid repeated allocations during traversal.
var textExtensions = map[string]bool{
	".txt": true, ".go": true, ".py": true, ".js": true, ".ts": true,
	".java": true, ".cpp": true, ".c": true, ".h": true, ".hpp": true,
	".cs": true, ".php": true, ".rb": true, ".rs": true, ".html": true,
	".css": true, ".xml": true, ".json": true, ".yaml": true, ".yml": true,
	".md": true, ".rst": true, ".sh": true, ".bat": true,
	".ps1": true, ".conf": true, ".cfg": true, ".ini": true,
}

var binaryExtensions = map[string]bool{
	".exe": true, ".dll": true, ".so": true, ".dylib": true,
	".bin": true, ".obj": true, ".lib": true, ".a": true,
	".o": true, ".lo": true, ".la": true,
	".jpg": true, ".jpeg": true, ".png": true, ".gif": true, ".bmp": true,
	".ico": true, ".tiff": true, ".tif": true, ".webp": true, ".psd": true,
	".raw": true, ".cr2": true, ".nef": true, ".svg": true,
	".pdf": true, ".doc": true, ".docx": true, ".xls": true, ".xlsx": true,
	".zip": true, ".tar": true, ".gz": true, ".7z": true, ".rar": true,
	".bz2": true, ".xz": true, ".lzma": true, ".cab": true, ".deb": true,
	".rpm": true, ".dmg": true, ".iso": true,
	".mp3": true, ".mp4": true, ".avi": true, ".mov": true, ".wav": true,
	".mkv": true, ".wmv": true, ".flv": true, ".webm": true, ".m4v": true,
	".m4a": true, ".flac": true, ".ogg": true, ".aac": true, ".wma": true,
	".pyc": true, ".pyo": true, ".class": true, ".jar": true, ".war": true,
	".ear": true, ".dex": true, ".apk": true, ".ipa": true,
	".cache": true, ".tmp": true, ".temp": true,
	".db": true, ".sqlite": true, ".sqlite3": true, ".mdb": true,
	".accdb": true, ".dbf": true,
	".ttf": true, ".otf": true, ".woff": true, ".woff2": true, ".eot": true,
	".p12": true, ".pfx": true, ".keystore": true, ".jks": true,
	".crt": true, ".der": true,
}

// SearchResult holds information about a specific pattern match within a file.
type SearchResult struct {
	FilePath  string // Full path to the file
	LineNum   int    // 1-based line number
	LineText  string // Full text of the line containing the match
	MatchText string // The specific text substring that triggered the regex
}

// fileJob encapsulates metadata for a file to be processed by a worker.
type fileJob struct {
	path string
	size int64
}

// ParseExcludeList converts a comma-separated string of patterns into a slice.
func ParseExcludeList(excludeStr string) []string {
	var out []string
	for _, part := range strings.Split(excludeStr, ",") {
		p := strings.TrimSpace(part)
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}

// IsExcluded checks if a given file or directory path matches any pattern in the exclusion list.
// It handles directory suffixes (ending with /), file extensions (*.ext), and exact filename matches.
func IsExcluded(path string, excludeList []string) bool {
	// Normalize path to use forward slashes for cross-platform pattern matching
	normalizedPath := filepath.ToSlash(path)

	for _, pattern := range excludeList {
		pattern = strings.TrimSpace(pattern)
		if pattern == "" {
			continue
		}

		// Directory exclusion
		if strings.HasSuffix(pattern, "/") {
			if strings.Contains(normalizedPath, pattern) || strings.HasSuffix(normalizedPath+"/", pattern) {
				return true
			}
			continue
		}

		// Extension exclusion
		if strings.HasPrefix(pattern, "*.") {
			if strings.HasSuffix(normalizedPath, pattern[1:]) {
				return true
			}
			continue
		}

		// Exact file name match
		fileName := filepath.Base(normalizedPath)
		if fileName == pattern {
			return true
		}
	}
	return false
}

// searchLinesOptimized scans the provided byte slice line by line using a scanner.
// It is optimized for smaller files already loaded in memory.
func searchLinesOptimized(path string, content []byte, pattern *regexp.Regexp) []SearchResult {
	var results []SearchResult

	scanner := bufio.NewScanner(bytes.NewReader(content))

	// Use a 128KB buffer with a 1MB limit per line to prevent scanning errors on long lines (e.g., minified files).
	buf := make([]byte, 128*1024)
	scanner.Buffer(buf, 1024*1024)

	lineNum := 0
	for scanner.Scan() {
		lineNum++
		lineBytes := scanner.Bytes()

		if pattern.Match(lineBytes) {
			matchBytes := pattern.Find(lineBytes)
			results = append(results, SearchResult{
				FilePath:  path,
				LineNum:   lineNum,
				LineText:  scanner.Text(),
				MatchText: string(matchBytes),
			})
		}
	}

	return results
}

// searchLargeFileOptimized processes large files from disk using a buffered reader.
// This prevents high memory usage by avoiding loading the entire file into RAM.
func searchLargeFileOptimized(path string, pattern *regexp.Regexp) []SearchResult {
	var results []SearchResult

	file, err := os.Open(path)
	if err != nil {
		return results
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	buf := make([]byte, 128*1024)
	scanner.Buffer(buf, 1024*1024)

	lineNum := 0
	for scanner.Scan() {
		lineNum++
		line := scanner.Bytes()

		if pattern.Match(line) {
			match := pattern.Find(line)
			results = append(results, SearchResult{
				FilePath:  path,
				LineNum:   lineNum,
				LineText:  string(line),
				MatchText: string(match),
			})
		}
	}

	return results
}

// searchSmallFileOptimized reads the file entirely and checks if the pattern exists
// before splitting it into lines, which is faster for small files.
func searchSmallFileOptimized(path string, pattern *regexp.Regexp) []SearchResult {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil
	}

	// Fast check: if the file doesn't match the regex at all, skip line-by-line processing.
	if !pattern.Match(content) {
		return nil
	}

	return searchLinesOptimized(path, content, pattern)
}

// extractLiteralPrefix attempts to find a static string at the beginning of a regex.
// This prefix can be used for fast pre-filtering with bytes.Contains.
func extractLiteralPrefix(regexStr string) string {
	// If case-insensitivity is enabled, we cannot use a simple bytes.Contains pre-filter
	// because it would miss matches with different casing.
	if strings.Contains(regexStr, "(?i)") {
		return ""
	}

	// Remove standard anchors
	regexStr = strings.TrimPrefix(regexStr, "^")
	regexStr = strings.TrimSuffix(regexStr, "$")
	regexStr = strings.TrimPrefix(regexStr, "(?m)")

	var literal strings.Builder
	escaped := false

	for _, r := range regexStr {
		if escaped {
			literal.WriteRune(r)
			escaped = false
			continue
		}

		switch r {
		case '\\':
			escaped = true
		case '.', '*', '+', '?', '^', '$', '(', ')', '[', ']', '{', '}', '|':
			// Stop at the first regex metacharacter to ensure the prefix is purely literal.
			return literal.String()
		default:
			literal.WriteRune(r)
		}
	}

	result := literal.String()
	// Only return if the literal part is long enough to provide a performance benefit.
	if len(result) >= 3 {
		return result
	}
	return ""
}

// isLikelyTextFile determines if a file should be searched based on its extension or content.
func isLikelyTextFile(path string) bool {
	ext := strings.ToLower(filepath.Ext(path))

	if _, ok := textExtensions[ext]; ok {
		return true
	}

	if binaryExtensions[ext] {
		return false
	}

	// If extension is unknown, fall back to checking the first few bytes for binary characters.
	return isTextFileContent(path)
}

// isTextFileContent reads the start of a file to check for null bytes or control characters.
func isTextFileContent(path string) bool {
	file, err := os.Open(path)
	if err != nil {
		return false
	}
	defer file.Close()

	buf := make([]byte, 512)
	n, err := file.Read(buf)
	if err != nil && n == 0 {
		return false
	}

	buf = buf[:n]

	for _, b := range buf {
		// 0 is null byte, < 32 are control characters (excluding Tab, LF, CR)
		if b == 0 || (b < 32 && b != 9 && b != 10 && b != 13) {
			return false
		}
	}

	return true
}

// PerformSearchAdaptive traverses a directory tree and searches for a regex pattern.
// It uses a pool of worker goroutines to process files in parallel and selects
// the optimal search strategy based on file size and regex complexity.
func PerformSearchAdaptive(rootPath string, pattern *regexp.Regexp, excludeList []string) ([]SearchResult, error) {
	literal := extractLiteralPrefix(pattern.String())
	hasLiteral := len(literal) >= 3

	var results []SearchResult
	fileCh := make(chan fileJob, 100)
	resultCh := make(chan []SearchResult, 100)

	numWorkers := runtime.NumCPU()
	if numWorkers < 1 {
		numWorkers = 1
	}

	var wg sync.WaitGroup

	// Worker Goroutines: Process files sent through fileCh.
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for job := range fileCh {
				var fileResults []SearchResult

				// Adaptive selection:
				// 1. Files > 10MB are streamed to save memory.
				// 2. If a literal prefix exists, use it to skip non-matching files quickly.
				// 3. Otherwise, use the standard optimized small-file search.
				if job.size > 10*1024*1024 {
					fileResults = searchLargeFileOptimized(job.path, pattern)
				} else if hasLiteral {
					fileResults = searchWithPrefilter(job.path, pattern, literal)
				} else {
					fileResults = searchSmallFileOptimized(job.path, pattern)
				}

				if len(fileResults) > 0 {
					resultCh <- fileResults
				}
			}
		}()
	}

	walkErrCh := make(chan error, 1)

	// Producer Goroutine: Recursively walks the directory.
	go func() {
		defer close(fileCh)
		err := filepath.WalkDir(rootPath, func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				return nil // Skip files/folders that cannot be accessed.
			}

			// Pre-emptive exclusion check: If a directory is excluded, skip it entirely.
			if IsExcluded(path, excludeList) {
				if d.IsDir() {
					return filepath.SkipDir
				}
				return nil
			}

			if d.IsDir() {
				// Hardcoded performance skips for common heavy directories.
				if d.Name() == ".git" || d.Name() == ".cache" || d.Name() == "node_modules" {
					return filepath.SkipDir
				}
				return nil
			}

			// Skip binary files and non-text formats.
			if !isLikelyTextFile(path) {
				return nil
			}

			info, err := d.Info()
			if err != nil {
				return nil
			}

			// Send the job to the worker pool.
			fileCh <- fileJob{path: path, size: info.Size()}
			return nil
		})
		walkErrCh <- err
	}()

	// Orchestration: Close resultCh once all workers are finished.
	go func() {
		wg.Wait()
		close(resultCh)
	}()

	// Collect results from the result channel.
	for fileResults := range resultCh {
		results = append(results, fileResults...)
	}

	// Capture any error from the directory walker.
	if err := <-walkErrCh; err != nil {
		return results, err
	}

	return results, nil
}

// searchWithPrefilter uses a fast literal string check before invoking the more expensive regex engine.
func searchWithPrefilter(path string, pattern *regexp.Regexp, literal string) []SearchResult {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil
	}

	// Performance boost: bytes.Contains is significantly faster than Regex Match.
	if !bytes.Contains(content, []byte(literal)) {
		return nil
	}

	if !pattern.Match(content) {
		return nil
	}

	return searchLinesOptimized(path, content, pattern)
}
