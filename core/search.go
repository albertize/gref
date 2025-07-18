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

// parseExcludeList parses comma separated exclude patterns
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

// Check if path matches any exclude pattern
func IsExcluded(path string, excludeList []string) bool {
	for _, pattern := range excludeList {
		pattern = strings.TrimSpace(pattern)
		if pattern == "" {
			continue
		}
		// Directory exclusion (ends with /)
		if strings.HasSuffix(pattern, "/") {
			if strings.Contains(path, pattern) {
				return true
			}
			continue
		}
		// Extension exclusion (starts with .)
		if strings.HasPrefix(pattern, "*.") {
			if strings.HasSuffix(path, pattern[1:]) {
				return true
			}
			continue
		}
		// File exclusion (exact match or contains)
		if strings.HasSuffix(path, pattern) || strings.Contains(path, pattern) {
			return true
		}
	}
	return false
}

// SearchResult holds information about a found match
type SearchResult struct {
	FilePath  string
	LineNum   int
	LineText  string
	MatchText string
}

// Avoid string conversion until needed
func searchLinesOptimized(path string, content []byte, pattern *regexp.Regexp) []SearchResult {
	var results []SearchResult

	// Using scanner in case of \r\n "windows style"
	scanner := bufio.NewScanner(bytes.NewReader(content))

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

// Process large files without loading entirely
func searchLargeFileOptimized(path string, pattern *regexp.Regexp) []SearchResult {
	var results []SearchResult

	file, err := os.Open(path)
	if err != nil {
		return results
	}
	defer file.Close()

	// Use larger buffer for better I/O performance
	scanner := bufio.NewScanner(file)
	buf := make([]byte, 128*1024)  // 128KB buffer
	scanner.Buffer(buf, 1024*1024) // 1MB max line length

	lineNum := 0
	for scanner.Scan() {
		lineNum++
		line := scanner.Text()

		if pattern.MatchString(line) {
			match := pattern.FindString(line)
			results = append(results, SearchResult{
				FilePath:  path,
				LineNum:   lineNum,
				LineText:  line,
				MatchText: match,
			})
		}
	}

	return results
}

// Read entirely and process efficiently
func searchSmallFileOptimized(path string, pattern *regexp.Regexp) []SearchResult {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil
	}

	// Fast pre-check on bytes
	if !pattern.Match(content) {
		return nil
	}

	return searchLinesOptimized(path, content, pattern)
}

// Extract literal part of regex for pre-filtering
func extractLiteralPrefix(regexStr string) string {
	// Remove common regex anchors and modifiers
	regexStr = strings.TrimPrefix(regexStr, "^")
	regexStr = strings.TrimSuffix(regexStr, "$")
	regexStr = strings.TrimPrefix(regexStr, "(?i)")
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
			// Stop at first regex metacharacter
			return literal.String()
		default:
			literal.WriteRune(r)
		}
	}

	result := literal.String()
	if len(result) >= 3 { // Only use if meaningful length
		return result
	}
	return ""
}

// 7. More sophisticated than just extension
func isLikelyTextFile(path string) bool {
	// First check by extension
	ext := strings.ToLower(filepath.Ext(path))
	textExtensions := map[string]bool{
		".txt": true, ".go": true, ".py": true, ".js": true, ".ts": true,
		".java": true, ".cpp": true, ".c": true, ".h": true, ".hpp": true,
		".cs": true, ".php": true, ".rb": true, ".rs": true, ".html": true,
		".css": true, ".xml": true, ".json": true, ".yaml": true, ".yml": true,
		".md": true, ".rst": true, ".sql": true, ".sh": true, ".bat": true,
		".ps1": true, ".log": true, ".conf": true, ".cfg": true, ".ini": true,
		"": true, // Files without extension
	}

	if textExtensions[ext] {
		return true
	}

	// Skip known binary extensions
	binaryExtensions := map[string]bool{
		// Executables and libraries
		".exe": true, ".dll": true, ".so": true, ".dylib": true,
		".bin": true, ".obj": true, ".lib": true, ".a": true,
		".o": true, ".lo": true, ".la": true,

		// Images
		".jpg": true, ".jpeg": true, ".png": true, ".gif": true, ".bmp": true,
		".ico": true, ".tiff": true, ".tif": true, ".webp": true, ".psd": true,
		".raw": true, ".cr2": true, ".nef": true, ".svg": true,

		// Documents
		".pdf": true, ".doc": true, ".docx": true, ".xls": true, ".xlsx": true,

		// Archives
		".zip": true, ".tar": true, ".gz": true, ".7z": true, ".rar": true,
		".bz2": true, ".xz": true, ".lzma": true, ".cab": true, ".deb": true,
		".rpm": true, ".dmg": true, ".iso": true,

		// Audio/Video
		".mp3": true, ".mp4": true, ".avi": true, ".mov": true, ".wav": true,
		".mkv": true, ".wmv": true, ".flv": true, ".webm": true, ".m4v": true,
		".m4a": true, ".flac": true, ".ogg": true, ".aac": true, ".wma": true,

		// Development/Build artifacts
		".pyc": true, ".pyo": true, ".class": true, ".jar": true, ".war": true,
		".ear": true, ".dex": true, ".apk": true, ".ipa": true,
		".cache": true, ".tmp": true, ".temp": true,

		// Database files
		".db": true, ".sqlite": true, ".sqlite3": true, ".mdb": true,
		".accdb": true, ".dbf": true,

		// Fonts
		".ttf": true, ".otf": true, ".woff": true, ".woff2": true, ".eot": true,

		// Certificates and keys
		".p12": true, ".pfx": true, ".keystore": true, ".jks": true,
		".crt": true, ".der": true,

		// Proprietary formats
		".sketch": true, ".fig": true, ".ai": true, ".eps": true, ".indd": true,

		// System files
		".sys": true, ".drv": true, ".reg": true, ".dat": true,
	}

	if binaryExtensions[ext] {
		return false
	}

	// For unknown extensions, do a quick binary check
	return isTextFileContent(path)
}

// Check first few bytes
func isTextFileContent(path string) bool {
	file, err := os.Open(path)
	if err != nil {
		return false
	}
	defer file.Close()

	// Read first 512 bytes
	buf := make([]byte, 512)
	n, err := file.Read(buf)
	if err != nil && n == 0 {
		return false
	}

	buf = buf[:n]

	// Check for binary content
	for _, b := range buf {
		if b == 0 || (b < 32 && b != 9 && b != 10 && b != 13) {
			return false // Contains null bytes or non-printable characters
		}
	}

	return true
}

// Choose best approach based on file characteristics and exclusions
func PerformSearchAdaptive(rootPath string, pattern *regexp.Regexp, excludeList []string) ([]SearchResult, error) {
	literal := extractLiteralPrefix(pattern.String())
	hasLiteral := len(literal) >= 3

	var results []SearchResult
	var mu sync.Mutex
	var wg sync.WaitGroup
	fileCh := make(chan string, 32)
	resultCh := make(chan []SearchResult, 32)

	// Autotune worker count based on OS and CPU
	cpuCount := runtime.NumCPU()
	osType := runtime.GOOS
	var numWorkers int
	if osType == "windows" {
		numWorkers = min(cpuCount, 4) // Windows: limit to 4 due to file handle limits
	} else {
		numWorkers = min(cpuCount*2, 32) // Unix: up to 2x CPUs, max 32
	}
	if numWorkers < 1 {
		numWorkers = 1
	}

	// Start workers
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for path := range fileCh {
				if IsExcluded(path, excludeList) {
					continue
				}
				// Skip all .git folders
				if strings.Contains(path, ".git") {
					continue
				}
				info, err := os.Stat(path)
				if err != nil {
					continue
				}
				var fileResults []SearchResult
				if info.Size() > 10*1024*1024 {
					fileResults = searchLargeFileOptimized(path, pattern)
				} else if hasLiteral {
					fileResults = searchWithPrefilter(path, pattern, literal)
				} else {
					fileResults = searchSmallFileOptimized(path, pattern)
				}
				if len(fileResults) > 0 {
					resultCh <- fileResults
				}
			}
		}()
	}

	// WalkDir and send file paths directly to fileCh
	walkErrCh := make(chan error, 1)
	go func() {
		err := filepath.WalkDir(rootPath, func(path string, d fs.DirEntry, err error) error {
			if err != nil || d.IsDir() {
				return nil
			}
			if IsExcluded(path, excludeList) {
				return nil
			}
			if !isLikelyTextFile(path) {
				return nil
			}
			fileCh <- path
			return nil
		})
		close(fileCh)
		walkErrCh <- err
	}()

	go func() {
		wg.Wait()
		close(resultCh)
	}()

	for fileResults := range resultCh {
		mu.Lock()
		results = append(results, fileResults...)
		mu.Unlock()
	}

	err := <-walkErrCh
	if err != nil {
		return nil, err
	}

	return results, nil
}

// Helper for pre-filter approach
func searchWithPrefilter(path string, pattern *regexp.Regexp, literal string) []SearchResult {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil
	}

	// Fast literal check
	if !bytes.Contains(content, []byte(literal)) {
		return nil
	}

	// Then regex check
	if !pattern.Match(content) {
		return nil
	}

	return searchLinesOptimized(path, content, pattern)
}
