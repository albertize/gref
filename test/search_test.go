package main

import (
	"testing"

	grefcore "github.com/albertize/gref/core"
)

func TestParseExcludeList(t *testing.T) {
	input := "foo, bar ,baz/"
	out := grefcore.ParseExcludeList(input)
	if len(out) != 3 || out[0] != "foo" || out[1] != "bar" || out[2] != "baz/" {
		t.Errorf("ParseExcludeList failed: got %v", out)
	}
}

func TestIsExcluded(t *testing.T) {
	exclude := []string{".git", "*.log", "media/", "file.txt"}
	cases := []struct {
		path     string
		expected bool
	}{
		{"/home/user/project/.git", true},
		{"/home/user/project/media/image.png", true},
		{"/home/user/project/file.txt", true},
		{"/home/user/project/notes.log", true},
		{"/home/user/project/notes.txt", false},
		{"/home/user/project/src/main.go", false},
	}
	for _, c := range cases {
		if grefcore.IsExcluded(c.path, exclude) != c.expected {
			t.Errorf("IsExcluded(%q) = %v, want %v", c.path, grefcore.IsExcluded(c.path, exclude), c.expected)
		}
	}
}
