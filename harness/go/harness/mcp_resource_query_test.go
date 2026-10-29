package harness

import (
	"context"
	"encoding/json"
	"reflect"
	"sort"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
	point "github.com/phodal/pero/harness/go/runtime"
)

// Resource and template catalogs are queried when the call runs. Tool catalogs
// refresh from notifications/tools/list_changed. This fixture leaves
// resources.listChanged off, so the next list/read on the same connection sees
// the mutated server because that call queries again.
func TestMCPResourcesQueryOnCallSeesFixtureChanges(t *testing.T) {
	ctx, cancel := context.WithTimeout(t.Context(), 10*time.Second)
	defer cancel()

	var note atomic.Value
	note.Store("first")
	var calls sync.Mutex
	counts := map[string]int{}
	server := mcp.NewServer(&mcp.Implementation{Name: "resource-query-fixture", Version: "1"}, &mcp.ServerOptions{
		Capabilities: &mcp.ServerCapabilities{
			Resources: &mcp.ResourceCapabilities{ListChanged: false},
		},
	})
	server.AddReceivingMiddleware(func(next mcp.MethodHandler) mcp.MethodHandler {
		return func(ctx context.Context, method string, req mcp.Request) (mcp.Result, error) {
			calls.Lock()
			counts[method]++
			calls.Unlock()
			return next(ctx, method, req)
		}
	})
	server.AddResource(&mcp.Resource{URI: "mem://note", Name: "note"}, func(context.Context, *mcp.ReadResourceRequest) (*mcp.ReadResourceResult, error) {
		return &mcp.ReadResourceResult{Contents: []*mcp.ResourceContents{{URI: "mem://note", Text: note.Load().(string)}}}, nil
	})
	server.AddResource(&mcp.Resource{URI: "mem://alpha", Name: "alpha"}, func(context.Context, *mcp.ReadResourceRequest) (*mcp.ReadResourceResult, error) {
		return &mcp.ReadResourceResult{Contents: []*mcp.ResourceContents{{URI: "mem://alpha", Text: "alpha"}}}, nil
	})
	server.AddResourceTemplate(&mcp.ResourceTemplate{URITemplate: "mem://item/{id}", Name: "item"}, func(_ context.Context, req *mcp.ReadResourceRequest) (*mcp.ReadResourceResult, error) {
		return &mcp.ReadResourceResult{Contents: []*mcp.ResourceContents{{URI: req.Params.URI, Text: "item"}}}, nil
	})

	clientTransport, serverTransport := mcp.NewInMemoryTransports()
	session, err := server.Connect(ctx, serverTransport, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })

	h := retirementHarness(t)
	agent, err := h.Agents().Create(ctx, AgentOptions{ID: "mcp-resource-query", Configure: func(ctx context.Context, run *point.Run, _ *Agent) error {
		return MountMCPServers(ctx, run, h.Tools(), h.Prompts(), []MCPServerConfig{{
			Name: "fixture", TestTransport: clientTransport, FailOnStartupError: true,
		}})
	}})
	if err != nil {
		t.Fatal(err)
	}

	if got := mcpListedResourceURIs(t, callMCPResource(t, ctx, h, agent, "list_mcp_resources", `{"server":"fixture"}`)); !reflect.DeepEqual(got, []string{"mem://alpha", "mem://note"}) {
		t.Fatalf("resources = %#v", got)
	}
	if got := mcpListedTemplateURIs(t, callMCPResource(t, ctx, h, agent, "list_mcp_resource_templates", `{"server":"fixture"}`)); !reflect.DeepEqual(got, []string{"mem://item/{id}"}) {
		t.Fatalf("templates = %#v", got)
	}
	if got := mcpReadResourceText(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://note"}`)); got != "first" {
		t.Fatalf("note = %q", got)
	}
	if got := mcpReadResourceText(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://alpha"}`)); got != "alpha" {
		t.Fatalf("alpha = %q", got)
	}
	if got := mcpReadResourceText(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://item/one"}`)); got != "item" {
		t.Fatalf("item = %q", got)
	}

	calls.Lock()
	listed, templates, reads := counts["resources/list"], counts["resources/templates/list"], counts["resources/read"]
	calls.Unlock()
	if listed < 1 || templates < 1 || reads < 1 {
		t.Fatalf("fixture was not queried: list=%d templates=%d read=%d", listed, templates, reads)
	}

	note.Store("second")
	server.RemoveResources("mem://alpha")
	server.AddResource(&mcp.Resource{URI: "mem://beta", Name: "beta"}, func(context.Context, *mcp.ReadResourceRequest) (*mcp.ReadResourceResult, error) {
		return &mcp.ReadResourceResult{Contents: []*mcp.ResourceContents{{URI: "mem://beta", Text: "beta"}}}, nil
	})
	server.RemoveResourceTemplates("mem://item/{id}")
	server.AddResourceTemplate(&mcp.ResourceTemplate{URITemplate: "mem://doc/{name}", Name: "doc"}, func(_ context.Context, req *mcp.ReadResourceRequest) (*mcp.ReadResourceResult, error) {
		return &mcp.ReadResourceResult{Contents: []*mcp.ResourceContents{{URI: req.Params.URI, Text: "doc"}}}, nil
	})

	if got := mcpListedResourceURIs(t, callMCPResource(t, ctx, h, agent, "list_mcp_resources", `{"server":"fixture"}`)); !reflect.DeepEqual(got, []string{"mem://beta", "mem://note"}) {
		t.Fatalf("resources after change = %#v", got)
	}
	if got := mcpListedTemplateURIs(t, callMCPResource(t, ctx, h, agent, "list_mcp_resource_templates", `{"server":"fixture"}`)); !reflect.DeepEqual(got, []string{"mem://doc/{name}"}) {
		t.Fatalf("templates after change = %#v", got)
	}
	if got := mcpReadResourceText(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://note"}`)); got != "second" {
		t.Fatalf("note after change = %q", got)
	}
	if got := mcpReadResourceText(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://beta"}`)); got != "beta" {
		t.Fatalf("beta = %q", got)
	}
	if got := mcpReadResourceText(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://doc/guide"}`)); got != "doc" {
		t.Fatalf("doc = %q", got)
	}
	requireMCPResourceMiss(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://alpha"}`))
	requireMCPResourceMiss(t, callMCPResource(t, ctx, h, agent, "read_mcp_resource", `{"server":"fixture","uri":"mem://item/one"}`))

	calls.Lock()
	defer calls.Unlock()
	if counts["resources/list"] != listed+1 || counts["resources/templates/list"] != templates+1 || counts["resources/read"] <= reads {
		t.Fatalf("queries after change: list %d->%d templates %d->%d read %d->%d",
			listed, counts["resources/list"], templates, counts["resources/templates/list"], reads, counts["resources/read"])
	}
}

func callMCPResource(t *testing.T, ctx context.Context, h *Harness, agent *Agent, name, args string) ToolOutcome {
	t.Helper()
	outcome, err := h.Tools().Execute(ctx, ToolRequest{Agent: agent, Name: name, Arguments: json.RawMessage(args)})
	if err != nil {
		t.Fatal(err)
	}
	return outcome
}

func mcpResourceValue(t *testing.T, outcome ToolOutcome) json.RawMessage {
	t.Helper()
	if outcome.Result.IsError {
		t.Fatalf("resource call failed: %+v", outcome.Result.Content)
	}
	var wrapped struct {
		Value json.RawMessage `json:"value"`
	}
	if err := json.Unmarshal(outcome.Result.Meta, &wrapped); err != nil {
		t.Fatalf("meta %s: %v", outcome.Result.Meta, err)
	}
	if len(wrapped.Value) == 0 {
		t.Fatalf("missing resource value: %s", outcome.Result.Meta)
	}
	return wrapped.Value
}

func mcpListedResourceURIs(t *testing.T, outcome ToolOutcome) []string {
	t.Helper()
	var page struct {
		Resources []struct {
			URI string `json:"uri"`
		} `json:"resources"`
	}
	if err := json.Unmarshal(mcpResourceValue(t, outcome), &page); err != nil {
		t.Fatal(err)
	}
	uris := make([]string, len(page.Resources))
	for i, resource := range page.Resources {
		uris[i] = resource.URI
	}
	sort.Strings(uris)
	return uris
}

func mcpListedTemplateURIs(t *testing.T, outcome ToolOutcome) []string {
	t.Helper()
	var page struct {
		Templates []struct {
			URITemplate string `json:"uriTemplate"`
		} `json:"resourceTemplates"`
	}
	if err := json.Unmarshal(mcpResourceValue(t, outcome), &page); err != nil {
		t.Fatal(err)
	}
	uris := make([]string, len(page.Templates))
	for i, template := range page.Templates {
		uris[i] = template.URITemplate
	}
	sort.Strings(uris)
	return uris
}

func mcpReadResourceText(t *testing.T, outcome ToolOutcome) string {
	t.Helper()
	var page struct {
		Contents []struct {
			Text string `json:"text"`
		} `json:"contents"`
	}
	if err := json.Unmarshal(mcpResourceValue(t, outcome), &page); err != nil {
		t.Fatal(err)
	}
	if len(page.Contents) != 1 {
		t.Fatalf("resource contents = %#v", page.Contents)
	}
	return page.Contents[0].Text
}

func requireMCPResourceMiss(t *testing.T, outcome ToolOutcome) {
	t.Helper()
	if !outcome.Result.IsError {
		t.Fatalf("removed resource still readable: %s", outcome.Result.Meta)
	}
}
