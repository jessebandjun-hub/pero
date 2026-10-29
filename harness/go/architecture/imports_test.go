package architecture_test

import (
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// boundary is one documented import allowlist.
//
// llm/README.md: dependencies are the sibling model package and the standard
// library. session/README.md: sibling model and runtime, plus golang.org/x/sys.
// model and runtime already enforce their standard-library gates in-package.
type boundary struct {
	name     string
	dir      string
	exact    map[string]struct{}
	prefixes []string
}

func (b boundary) allows(importPath string) bool {
	if isStdlib(importPath) {
		return true
	}
	if _, ok := b.exact[importPath]; ok {
		return true
	}
	for _, prefix := range b.prefixes {
		if strings.HasPrefix(importPath, prefix) {
			return true
		}
	}
	return false
}

func isStdlib(importPath string) bool {
	first, _, _ := strings.Cut(importPath, "/")
	return !strings.Contains(first, ".")
}

func documentedBoundaries() []boundary {
	return []boundary{
		{
			name: "llm",
			dir:  "llm",
			exact: map[string]struct{}{
				"github.com/phodal/pero/harness/go/model": {},
			},
		},
		{
			name: "session",
			dir:  "session",
			exact: map[string]struct{}{
				"github.com/phodal/pero/harness/go/model":   {},
				"github.com/phodal/pero/harness/go/runtime": {},
			},
			prefixes: []string{"golang.org/x/sys/"},
		},
	}
}

func boundaryByName(t *testing.T, name string) boundary {
	t.Helper()
	for _, item := range documentedBoundaries() {
		if item.name == name {
			return item
		}
	}
	t.Fatalf("unknown boundary %s", name)
	return boundary{}
}

func TestImportBoundaryDecisions(t *testing.T) {
	// Rejection rows are the red contract. The production scan uses allows.
	cases := []struct {
		pkg   string
		path  string
		allow bool
	}{
		{"llm", "fmt", true},
		{"llm", "github.com/phodal/pero/harness/go/model", true},
		{"llm", "github.com/phodal/pero/harness/go/session", false},
		{"llm", "github.com/phodal/pero/harness/go/harness", false},
		{"llm", "github.com/phodal/pero/harness/go/acp", false},
		{"llm", "golang.org/x/sys/unix", false},
		{"session", "runtime", true},
		{"session", "github.com/phodal/pero/harness/go/model", true},
		{"session", "github.com/phodal/pero/harness/go/runtime", true},
		{"session", "golang.org/x/sys/unix", true},
		{"session", "golang.org/x/sys/windows", true},
		{"session", "github.com/phodal/pero/harness/go/llm", false},
		{"session", "github.com/phodal/pero/harness/go/harness", false},
		{"session", "github.com/phodal/pero/harness/go/acp", false},
		{"session", "github.com/openai/openai-go/v3", false},
		{"session", "golang.org/x/text/unicode/norm", false},
	}
	for _, tc := range cases {
		got := boundaryByName(t, tc.pkg).allows(tc.path)
		if got != tc.allow {
			t.Errorf("%s import %s: allowed=%v, want %v", tc.pkg, tc.path, got, tc.allow)
		}
	}
}

func TestScannerRejectsForbiddenImport(t *testing.T) {
	rule := boundaryByName(t, "llm")
	cases := []struct {
		name       string
		file       string
		source     string
		inspected  int
		violations int
	}{
		{
			name:       "sibling session import",
			file:       "bad.go",
			source:     "package llm\nimport \"github.com/phodal/pero/harness/go/session\"\n",
			inspected:  1,
			violations: 1,
		},
		{
			name:       "documented model import",
			file:       "ok.go",
			source:     "package llm\nimport (\n\t\"fmt\"\n\t\"github.com/phodal/pero/harness/go/model\"\n)\n",
			inspected:  1,
			violations: 0,
		},
		{
			name:       "tests are outside the package allowlist",
			file:       "bad_test.go",
			source:     "package llm\nimport \"github.com/phodal/pero/harness/go/harness\"\n",
			inspected:  0,
			violations: 0,
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			if err := os.WriteFile(filepath.Join(dir, tc.file), []byte(tc.source), 0o644); err != nil {
				t.Fatal(err)
			}
			inspected, violations, err := scanProductionImports(dir, rule)
			if err != nil {
				t.Fatal(err)
			}
			if inspected != tc.inspected || len(violations) != tc.violations {
				t.Fatalf("inspected=%d violations=%v, want inspected=%d violations=%d", inspected, violations, tc.inspected, tc.violations)
			}
		})
	}
}

func TestProductionSourcesMatchImportBoundaries(t *testing.T) {
	root := harnessRoot(t)
	for _, rule := range documentedBoundaries() {
		inspected, violations, err := scanProductionImports(filepath.Join(root, rule.dir), rule)
		if err != nil {
			t.Fatal(err)
		}
		if inspected == 0 {
			t.Fatalf("%s: no production Go sources available for dependency inspection", rule.name)
		}
		for _, violation := range violations {
			t.Error(violation)
		}
	}
}

func scanProductionImports(dir string, rule boundary) (int, []string, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return 0, nil, err
	}
	inspected := 0
	var violations []string
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".go") || strings.HasSuffix(entry.Name(), "_test.go") {
			continue
		}
		inspected++
		path := filepath.Join(dir, entry.Name())
		file, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
		if err != nil {
			return inspected, violations, err
		}
		for _, spec := range file.Imports {
			importPath, err := strconv.Unquote(spec.Path.Value)
			if err != nil {
				return inspected, violations, err
			}
			if !rule.allows(importPath) {
				violations = append(violations, rule.name+"/"+entry.Name()+" imports "+importPath)
			}
		}
	}
	return inspected, violations, nil
}

func TestModuleHasNoReplaceDirective(t *testing.T) {
	if !moduleHasReplace("module m\n\nreplace example.com/x => ../x\n") {
		t.Fatal("a replace directive must be rejected")
	}
	if !moduleHasReplace("module m\nreplace (\n\texample.com/x => ../x\n)\n") {
		t.Fatal("a replace block must be rejected")
	}
	if moduleHasReplace("module m\n// replace example.com/x => ../x\nrequire example.com/x v1.0.0\n") {
		t.Fatal("a comment must not count as a replace directive")
	}
	text, err := os.ReadFile(filepath.Join(harnessRoot(t), "go.mod"))
	if err != nil {
		t.Fatal(err)
	}
	if moduleHasReplace(string(text)) {
		t.Fatal("harness/go/go.mod must not contain a replace directive")
	}
}

func moduleHasReplace(text string) bool {
	for _, line := range strings.Split(text, "\n") {
		field := strings.TrimSpace(line)
		if strings.HasPrefix(field, "replace ") || field == "replace" || strings.HasPrefix(field, "replace(") {
			return true
		}
	}
	return false
}
