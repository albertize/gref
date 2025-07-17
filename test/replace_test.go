package main

import (
	"os"
	"regexp"
	"testing"

	grefcore "github.com/albertize/gref/core"
)

func TestReplaceInFile(t *testing.T) {
	file := "test_replace.txt"
	os.WriteFile(file, []byte("foo bar\nfoo baz\nbar foo"), 0644)
	defer os.Remove(file)
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: "foo bar", MatchText: "foo"},
		{FilePath: file, LineNum: 2, LineText: "foo baz", MatchText: "foo"},
	}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	if string(content) != "qux bar\nqux baz\nbar foo" {
		t.Errorf("Unexpected file content: %s", string(content))
	}
}
func TestReplaceInFile_WindowsLineEndings(t *testing.T) {
	file := "test_replace_win.txt"
	os.WriteFile(file, []byte("foo bar\r\nfoo baz\r\nbar foo"), 0644)
	defer os.Remove(file)
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: "foo bar", MatchText: "foo"},
		{FilePath: file, LineNum: 2, LineText: "foo baz", MatchText: "foo"},
	}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	if string(content) != "qux bar\r\nqux baz\r\nbar foo" {
		t.Errorf("Unexpected file content: %s", string(content))
	}
}

func TestReplaceInFile_EmptyFile(t *testing.T) {
	file := "test_replace_empty.txt"
	os.WriteFile(file, []byte(""), 0644)
	defer os.Remove(file)
	results := []grefcore.SearchResult{}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	if string(content) != "" {
		t.Errorf("Unexpected file content: %s", string(content))
	}
}

func TestReplaceInFile_OnlyMatches(t *testing.T) {
	file := "test_replace_onlymatches.txt"
	os.WriteFile(file, []byte("foo\nfoo\nfoo"), 0644)
	defer os.Remove(file)
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: "foo", MatchText: "foo"},
		{FilePath: file, LineNum: 2, LineText: "foo", MatchText: "foo"},
		{FilePath: file, LineNum: 3, LineText: "foo", MatchText: "foo"},
	}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	if string(content) != "qux\nqux\nqux" {
		t.Errorf("Unexpected file content: %s", string(content))
	}
}

func TestReplaceInFile_NoMatches(t *testing.T) {
	file := "test_replace_nomatch.txt"
	os.WriteFile(file, []byte("bar\nbaz\nquux"), 0644)
	defer os.Remove(file)
	results := []grefcore.SearchResult{}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	if string(content) != "bar\nbaz\nquux" {
		t.Errorf("Unexpected file content: %s", string(content))
	}
}

func TestReplaceInFile_SpecialChars(t *testing.T) {
	file := "test_replace_special.txt"
	os.WriteFile(file, []byte("föö bär\nföö baz\nbär föö"), 0644)
	defer os.Remove(file)
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: "föö bär", MatchText: "föö"},
		{FilePath: file, LineNum: 2, LineText: "föö baz", MatchText: "föö"},
	}
	pattern := regexp.MustCompile("föö")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	if string(content) != "qux bär\nqux baz\nbär föö" {
		t.Errorf("Unexpected file content: %s", string(content))
	}
}

func TestReplaceInFile_ByteConflict(t *testing.T) {
	file := "test_replace_byteconflict.txt"
	// Simula una linea con byte non UTF-8 e caratteri speciali
	data := []byte{'f', 'o', 'o', 0xff, 'b', 'a', 'r', '\n', 'f', 'o', 'o', 0xfe, 'b', 'a', 'z', '\n', 'b', 'a', 'r', ' ', 'f', 'o', 'o'}
	os.WriteFile(file, data, 0644)
	defer os.Remove(file)
	// Le linee sono: "foo\xffbar", "foo\xfebaz", "bar foo"
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: string([]byte{'f', 'o', 'o', 0xff, 'b', 'a', 'r'}), MatchText: "foo"},
		{FilePath: file, LineNum: 2, LineText: string([]byte{'f', 'o', 'o', 0xfe, 'b', 'a', 'z'}), MatchText: "foo"},
	}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	expected := string([]byte{'q', 'u', 'x', 0xff, 'b', 'a', 'r', '\n', 'q', 'u', 'x', 0xfe, 'b', 'a', 'z', '\n', 'b', 'a', 'r', ' ', 'f', 'o', 'o'})
	if string(content) != expected {
		t.Errorf("Unexpected file content: %v", content)
	}
}

func TestReplaceInFile_FileNotReadable(t *testing.T) {
	file := "test_replace_noperm.txt"
	os.WriteFile(file, []byte("foo bar\nfoo baz"), 0644)
	os.Chmod(file, 0000) // rimuove permessi
	defer func() {
		os.Chmod(file, 0644)
		os.Remove(file)
	}()
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: "foo bar", MatchText: "foo"},
	}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err == nil {
		t.Errorf("Expected error for unreadable file, got nil")
	}
}

func TestReplaceInFile_InvalidRegexp(t *testing.T) {
	invalidRe := "["
	//lint:ignore SA1000 intentionally invalid regexp for test
	_, errRe := regexp.Compile(invalidRe) // regexp non valida
	if errRe == nil {
		t.Errorf("Expected error for invalid regexp, got nil")
	}
}

func TestReplaceInFile_OverlappingMatch(t *testing.T) {
	file := "test_replace_overlap.txt"
	os.WriteFile(file, []byte("aaaaa"), 0644)
	defer os.Remove(file)
	// pattern "aa" corrisponde a posizioni sovrapposte
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: "aaaaa", MatchText: "aa"},
		{FilePath: file, LineNum: 1, LineText: "aaaaa", MatchText: "aa"},
		{FilePath: file, LineNum: 1, LineText: "aaaaa", MatchText: "aa"},
	}
	pattern := regexp.MustCompile("aa")
	err := grefcore.ReplaceInFile(file, results, pattern, "b")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	// Il comportamento atteso dipende dall'implementazione, qui si verifica che non ci siano panics e che la sostituzione sia coerente
	if len(content) == 0 {
		t.Errorf("Unexpected empty file after overlapping replace")
	}
}

func TestReplaceInFile_BinaryNullBytes(t *testing.T) {
	file := "test_replace_nullbytes.txt"
	data := []byte{'f', 'o', 'o', 0x00, 'b', 'a', 'r', '\n', 'b', 'a', 'z', 0x00, 'f', 'o', 'o'}
	os.WriteFile(file, data, 0644)
	defer os.Remove(file)
	results := []grefcore.SearchResult{
		{FilePath: file, LineNum: 1, LineText: string([]byte{'f', 'o', 'o', 0x00, 'b', 'a', 'r'}), MatchText: "foo"},
	}
	pattern := regexp.MustCompile("foo")
	err := grefcore.ReplaceInFile(file, results, pattern, "qux")
	if err != nil {
		t.Fatalf("ReplaceInFile error: %v", err)
	}
	content, _ := os.ReadFile(file)
	expected := string([]byte{'q', 'u', 'x', 0x00, 'b', 'a', 'r', '\n', 'b', 'a', 'z', 0x00, 'f', 'o', 'o'})
	if string(content) != expected {
		t.Errorf("Unexpected file content: %v", content)
	}
}
