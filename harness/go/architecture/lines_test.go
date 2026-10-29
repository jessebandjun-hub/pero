package architecture_test

import (
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// maxSourceLines is the code-file ceiling in AGENTS.md, tests included.
// Nothing in the tree is over it, so there is no grandfather list.
const maxSourceLines = 1000

var sourceExt = map[string]struct{}{
	".go": {}, ".rs": {}, ".ts": {}, ".tsx": {}, ".js": {}, ".mjs": {}, ".py": {},
}

var skipDirs = map[string]struct{}{
	".git": {}, "node_modules": {}, "target": {}, "dist": {}, "vendor": {},
	"third_party": {}, ".aspect": {}, ".official-dsh-reference": {},
}

func TestLineCeilingContract(t *testing.T) {
	if got := countLines(strings.Repeat(".\n", 1000)); got != 1000 {
		t.Fatalf("1000 terminated lines counted as %d", got)
	}
	if got := countLines(strings.Repeat(".\n", 1001)); got <= maxSourceLines {
		t.Fatalf("1001 lines counted as %d and must fail the ceiling", got)
	}
	if countLines("") != 0 || countLines("a") != 1 || countLines("a\nb") != 2 || countLines("a\nb\n") != 2 {
		t.Fatalf("line count mismatch: empty=%d a=%d a\\nb=%d a\\nb\\n=%d", countLines(""), countLines("a"), countLines("a\nb"), countLines("a\nb\n"))
	}
}

func TestSourceFilesStayWithinLineCeiling(t *testing.T) {
	root, ok := checkoutRoot(t)
	if !ok {
		if os.Getenv("TEST_SRCDIR") == "" && os.Getenv("RUNFILES_DIR") == "" {
			t.Fatal("repository root not found")
		}
		// Bazel tests this module from harness/go. The harness go test job
		// checks the full checkout, including the Rust files nearest the ceiling.
		t.Log("repository checkout is outside the Bazel sandbox; go test from a checkout enforces the file ceiling")
		return
	}
	scanned := 0
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			name := entry.Name()
			if _, skip := skipDirs[name]; skip || strings.HasPrefix(name, "bazel-") {
				return filepath.SkipDir
			}
			return nil
		}
		if _, ok := sourceExt[filepath.Ext(entry.Name())]; !ok {
			return nil
		}
		text, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		scanned++
		if lines := countLines(string(text)); lines > maxSourceLines {
			rel, relErr := filepath.Rel(root, path)
			if relErr != nil {
				rel = path
			}
			t.Errorf("%s has %d lines; the ceiling is %d", filepath.ToSlash(rel), lines, maxSourceLines)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if scanned < 50 {
		t.Fatalf("scanned %d source files; the checkout walk did not see the tree", scanned)
	}
}

func countLines(text string) int {
	if text == "" {
		return 0
	}
	n := strings.Count(text, "\n")
	if !strings.HasSuffix(text, "\n") {
		n++
	}
	return n
}
