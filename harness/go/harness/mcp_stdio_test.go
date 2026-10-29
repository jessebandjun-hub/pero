package harness

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"

	"github.com/modelcontextprotocol/go-sdk/mcp"
	point "github.com/phodal/pero/harness/go/runtime"
)

// These names must survive mergedEnvironment scrubbing (no KEY/TOKEN/SECRET/PASSWORD/POINT_).
const (
	mcpStdioFixtureEnv = "PERO_MCP_STDIO_FIXTURE"
	mcpStdioPIDFileEnv = "PERO_MCP_STDIO_PIDFILE"
)

func TestMain(m *testing.M) {
	if os.Getenv(mcpStdioFixtureEnv) == "1" {
		runMCPStdioFixture()
		return
	}
	os.Exit(m.Run())
}

// runMCPStdioFixture speaks MCP on stdin/stdout. It is the command started by
// CommandTransport; test output would corrupt the stdio session.
func runMCPStdioFixture() {
	if path := os.Getenv(mcpStdioPIDFileEnv); path != "" {
		if err := os.WriteFile(path, []byte(strconv.Itoa(os.Getpid())), 0o600); err != nil {
			fmt.Fprintf(os.Stderr, "stdio fixture pid: %v\n", err)
			os.Exit(1)
		}
	}
	server := mcp.NewServer(&mcp.Implementation{Name: "stdio-fixture", Version: "1"}, nil)
	server.AddTool(&mcp.Tool{
		Name:        "echo",
		Description: "echo one string",
		InputSchema: json.RawMessage(`{"type":"object","additionalProperties":false,"required":["text"],"properties":{"text":{"type":"string"}}}`),
	}, func(_ context.Context, req *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		var args struct {
			Text string `json:"text"`
		}
		if req == nil || req.Params == nil || json.Unmarshal(req.Params.Arguments, &args) != nil || args.Text == "" {
			return nil, errors.New("stdio fixture requires text")
		}
		if args.Text == "fail" {
			return &mcp.CallToolResult{IsError: true, Content: []mcp.Content{&mcp.TextContent{Text: "stdio-error"}}}, nil
		}
		return &mcp.CallToolResult{Content: []mcp.Content{&mcp.TextContent{Text: "stdio:" + args.Text}}}, nil
	})
	// Client close ends Run with an error such as "server is closing: EOF".
	// Exit 0 so CommandTransport records a normal subprocess shutdown.
	_ = server.Run(context.Background(), &mcp.StdioTransport{})
}

// TestMCPStdioDiscoverAndCall lists and calls a tool through MountMCPServers
// and the production stdio CommandTransport. The command is this test binary's
// absolute path; TestMain serves the fixture when the env is set.
func TestMCPStdioDiscoverAndCall(t *testing.T) {
	exe, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	if !filepath.IsAbs(exe) {
		t.Fatalf("test binary is not absolute: %s", exe)
	}
	pidFile := filepath.Join(t.TempDir(), "pid")
	reconnect := false
	h := retirementHarness(t)
	mountRetirementHooks(t, h, func(run *point.Run) error {
		return MountMCPServers(t.Context(), run, h.Tools(), h.Prompts(), []MCPServerConfig{{
			Name:               "stdio_fixture",
			Transport:          MCPStdio,
			Command:            exe,
			Env:                map[string]string{mcpStdioFixtureEnv: "1", mcpStdioPIDFileEnv: pidFile},
			FailOnStartupError: true,
			Reconnect:          &MCPReconnectConfig{Enabled: &reconnect},
		}})
	})
	agent, err := h.Agents().Create(t.Context(), AgentOptions{ID: "mcp-stdio"})
	if err != nil {
		t.Fatal(err)
	}
	toolName := PublicMCPToolName("stdio_fixture", "echo")
	if _, ok := h.Tools().Resolve(agent.Scope(), toolName); !ok {
		t.Fatal("stdio tools/list did not publish echo")
	}
	outcome, err := h.Tools().Execute(t.Context(), ToolRequest{
		Agent:     agent,
		Name:      toolName,
		Arguments: json.RawMessage(`{"text":"ping"}`),
	})
	if err != nil {
		t.Fatal(err)
	}
	out := outcome.Result
	if out.IsError || len(out.Content) != 1 || out.Content[0].Type != BlockText || out.Content[0].Text != "stdio:ping" {
		t.Fatalf("tools/call = %+v", out)
	}
	if !strings.Contains(string(out.Value), "stdio:ping") {
		t.Fatalf("canonical value = %s", out.Value)
	}
	failed, err := h.Tools().Execute(t.Context(), ToolRequest{
		Agent:     agent,
		Name:      toolName,
		Arguments: json.RawMessage(`{"text":"fail"}`),
	})
	if err != nil {
		t.Fatal(err)
	}
	if !failed.Result.IsError || failed.Result.Value != nil || len(failed.Result.Content) != 1 || failed.Result.Content[0].Text != "stdio-error" {
		t.Fatalf("tools/call error = %+v", failed.Result)
	}
	if err := h.Dispose(context.Background()); err != nil {
		t.Fatal(err)
	}
	assertMCPStdioChildExited(t, pidFile)
}

func assertMCPStdioChildExited(t *testing.T, pidFile string) {
	t.Helper()
	raw, err := os.ReadFile(pidFile)
	if err != nil {
		t.Fatal(err)
	}
	pid, err := strconv.Atoi(strings.TrimSpace(string(raw)))
	if err != nil || pid <= 0 {
		t.Fatalf("stdio pid = %q", raw)
	}
	if runtime.GOOS != "linux" {
		return
	}
	environ, err := os.ReadFile("/proc/" + strconv.Itoa(pid) + "/environ")
	if err == nil && bytes.Contains(environ, []byte(mcpStdioFixtureEnv+"=1")) {
		t.Fatalf("stdio child %d still running after shutdown", pid)
	}
}
