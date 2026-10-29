package architecture_test

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func harnessRoot(t *testing.T) string {
	t.Helper()
	candidates := []string{}
	if cwd, err := os.Getwd(); err == nil {
		candidates = append(candidates, cwd, filepath.Clean(filepath.Join(cwd, "..")))
	}
	_, file, _, ok := runtime.Caller(0)
	if ok {
		candidates = append(candidates, filepath.Clean(filepath.Join(filepath.Dir(file), "..")))
	}
	workspaces := []string{os.Getenv("TEST_WORKSPACE"), "_main", "pero_agent"}
	for _, base := range []string{os.Getenv("RUNFILES_DIR"), os.Getenv("TEST_SRCDIR")} {
		if base == "" {
			continue
		}
		for _, ws := range workspaces {
			if ws == "" {
				continue
			}
			candidates = append(candidates, filepath.Join(base, ws))
		}
	}
	seen := map[string]bool{}
	for _, candidate := range candidates {
		if seen[candidate] {
			continue
		}
		seen[candidate] = true
		if _, err := os.Stat(filepath.Join(candidate, "go.mod")); err != nil {
			continue
		}
		if _, err := os.Stat(filepath.Join(candidate, "llm")); err != nil {
			continue
		}
		return candidate
	}
	t.Fatalf("harness module root not found; looked in %v", candidates)
	return ""
}

// checkoutRoot is the repository that contains ROADMAP.md. The Bazel sandbox
// for this module is only harness/go, so the full checkout is absent there.
func checkoutRoot(t *testing.T) (string, bool) {
	t.Helper()
	root := filepath.Clean(filepath.Join(harnessRoot(t), "..", ".."))
	if _, err := os.Stat(filepath.Join(root, "ROADMAP.md")); err != nil {
		return "", false
	}
	if _, err := os.Stat(filepath.Join(root, "harness", "go", "go.mod")); err != nil {
		return "", false
	}
	return root, true
}
