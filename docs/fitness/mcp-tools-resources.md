---
kind: verification_report
date: 2026-10-29
---

# MCP 工具与资源 acceptance

ROADMAP item 「MCP 工具与资源」 stays unchecked. Acceptance is stdio and
Streamable HTTP discover and call, a resource list that updates after a change,
and tool list updates. This note records L0 evidence. It does not mark the item complete.

## Tool list updates

`notifications/tools/list_changed` has a harness contract in
[phodal/pero#16](https://github.com/phodal/pero/pull/16):
`TestMCPToolListChangedUpdatesRegisteredTools`. The test mounts an in-memory
server and requires add, remove, and description replacement to refresh the
agent-visible tool generation. That contract was merged in #16, and it
leaves this ROADMAP checkbox open.

## Resource query on call

Tool catalogs refresh from `notifications/tools/list_changed`. Resource and
template catalogs are queried on the call: `list_mcp_resources`,
`list_mcp_resource_templates`, and `read_mcp_resource` invoke
`ListResources`, `ListResourceTemplates`, and `ReadResource` on the live
session.

`TestMCPResourcesQueryOnCallSeesFixtureChanges` keeps one in-memory
connection. It lists resources and templates, the fixture then replaces
both and changes a resource body, and the next list and read return that
state. The fixture sets `resources.listChanged` to false, and its results
use the default `ttlMs` of 0, so each call reaches the server. This
contract does not subscribe to `notifications/resources/list_changed` and
does not claim a notification-driven resource cache. A positive `ttlMs`
cache is outside this contract.

工具目录通知刷新；资源目录按调用查询。

## Streamable HTTP on master

`master` already exercises Streamable HTTP:

- `TestCodingModelCLICombinedActivationWire` attaches a loopback Streamable
  HTTP server at `session/new` and requires `remote_probe` in the provider
  tool list.
- `TestNativeApprovalMetadataWireAuthority` (`mcp_spoof`) performs one
  Streamable HTTP tool call after allow-once, and rejects native-approval
  metadata supplied by that server.
- `TestMCPHTTPRedirectDoesNotForwardConfiguredCredentials`,
  `TestMCPHTTPAuthenticationIsTerminalAndDiagnosticsAreRedacted`, and
  `TestMCPHTTPTransientFailureStillReconnectsWithoutLeakingBody` cover
  credential redirects, terminal authentication, and reconnect.

`TestMCPCanonicalOutput` exercises `tools/list` and `tools/call` through an
in-memory transport.

## Open follow-ups

- A stdio fixture that discovers and calls a tool.
