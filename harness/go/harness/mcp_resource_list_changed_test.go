package harness

import (
	"context"
	"encoding/json"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
	point "github.com/phodal/pero/harness/go/runtime"
)

// resources/list_changed must refresh the page list_mcp_resources returns.
// The fixture caches that page for a minute, so a later call stays on the
// previous resources until the session receives the notification. The server
// speaks the SDK's in-memory transport; no subprocess or network.
func TestMCPResourceListChangedUpdatesListedResources(t *testing.T) {
	ctx, cancel := context.WithTimeout(t.Context(), 10*time.Second)
	defer cancel()

	server := mcp.NewServer(&mcp.Implementation{Name: "resource-list-fixture", Version: "1"}, nil)
	server.AddReceivingMiddleware(func(next mcp.MethodHandler) mcp.MethodHandler {
		return func(ctx context.Context, method string, req mcp.Request) (mcp.Result, error) {
			result, err := next(ctx, method, req)
			if err == nil && method == "resources/list" {
				if page, ok := result.(*mcp.ListResourcesResult); ok {
					page.TTLMs = 60_000
				}
			}
			return result, err
		}
	})
	const alpha = "mem://alpha"
	const beta = "mem://beta"
	server.AddResource(&mcp.Resource{URI: alpha, Name: "alpha"}, nil)
	clientTransport, serverTransport := mcp.NewInMemoryTransports()
	session, err := server.Connect(ctx, serverTransport, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })

	h := retirementHarness(t)
	agent, err := h.Agents().Create(ctx, AgentOptions{ID: "mcp-resource-list-changed", Configure: func(ctx context.Context, run *point.Run, _ *Agent) error {
		return MountMCPServers(ctx, run, h.Tools(), h.Prompts(), []MCPServerConfig{{
			Name: "fixture", TestTransport: clientTransport, FailOnStartupError: true,
		}})
	}})
	if err != nil {
		t.Fatal(err)
	}

	if got := listFixtureResources(t, ctx, h, agent); !reflect.DeepEqual(got, []string{alpha}) {
		t.Fatalf("listed resources = %#v, want [%s]", got, alpha)
	}

	server.RemoveResources(alpha)
	server.AddResource(&mcp.Resource{URI: beta, Name: "beta"}, nil)
	waitForFixtureResources(t, ctx, h, agent, []string{beta})
}

func listFixtureResources(t *testing.T, ctx context.Context, h *Harness, agent *Agent) []string {
	t.Helper()
	outcome, err := h.Tools().Execute(ctx, ToolRequest{
		Agent: agent, Name: "list_mcp_resources", Arguments: json.RawMessage(`{"server":"fixture"}`),
	})
	if err != nil {
		t.Fatal(err)
	}
	if outcome.Result.IsError || len(outcome.Result.Content) != 1 {
		t.Fatalf("list_mcp_resources = %+v", outcome.Result)
	}
	payload, ok := strings.CutPrefix(outcome.Result.Content[0].Text, "MCP server: fixture\n")
	if !ok {
		t.Fatalf("list projection = %q", outcome.Result.Content[0].Text)
	}
	var body struct {
		Resources []struct {
			URI string `json:"uri"`
		} `json:"resources"`
	}
	if err := json.Unmarshal([]byte(payload), &body); err != nil {
		t.Fatalf("list payload %q: %v", payload, err)
	}
	uris := make([]string, len(body.Resources))
	for i, resource := range body.Resources {
		uris[i] = resource.URI
	}
	return uris
}

func waitForFixtureResources(t *testing.T, ctx context.Context, h *Harness, agent *Agent, want []string) {
	t.Helper()
	deadline, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	ticker := time.NewTicker(5 * time.Millisecond)
	defer ticker.Stop()
	var got []string
	for {
		got = listFixtureResources(t, ctx, h, agent)
		if reflect.DeepEqual(got, want) {
			return
		}
		select {
		case <-deadline.Done():
			t.Fatalf("listed resources = %#v, want %#v", got, want)
		case <-ticker.C:
		}
	}
}
