---
kind: verification_report
date: 2026-10-29
---

# Standalone module tests

ROADMAP 「独立模块验证」 stays unchecked. Acceptance is that each module can
run its tests after the monorepo workspace configuration is turned off, and
that real wiring stays distinct from a stand-in. This note records one
enforced Go gate on parent `604cca1`. It does not mark the item complete.

There is no root `go.work`. `GOWORK=off` disables Go workspace mode only. A
test can still open a parent file, which is why the gate copies the module
out of the checkout.

## Inventory

| Module | Workspace coupling | Detached tests |
| --- | --- | --- |
| `harness/go` | Own `go.mod`. Root Bazel ignores it (`.bazelignore`). Its nested Bazel module is a separate CI job and still runs inside the checkout. | Gated |
| `server` | Own `go.mod`. `TestRepositoryRoadmapParsesWithoutDiscardingCheckboxes` reads `../../../ROADMAP.md`. | Not gated |
| `crates/*` | Cargo workspace inheritance (`version.workspace`, `edition.workspace`, path dependencies). | Not gated |
| `desktop`, `web` | npm and pnpm workspace members. Neither package has a test script. Root `npm test` runs the Cargo workspace suite. | No module-local suite |

A detached `crates/pero-contracts` fails before compilation:

```text
error: failed to parse manifest
error inheriting `edition` from workspace root manifest's `workspace.package.edition`
failed to find a workspace root
```

A detached `server` copy, with `GOWORK=off` and Go 1.25.0, fails in the
roadmap test before PostgreSQL:

```text
--- FAIL: TestRepositoryRoadmapParsesWithoutDiscardingCheckboxes (0.00s)
    roadmap_test.go:93: open ../../../ROADMAP.md: no such file or directory
```

PostgreSQL cases in `server` stay real-or-skip. Without
`PERO_TEST_DATABASE_URL` they skip; `PERO_REQUIRE_TEST_DATABASE=1` makes a
missing database a failure. This gate does not stub that database.

## Gate

`scripts/check/standalone-go-module.sh` copies one Go module to a temporary
directory outside the repository, exports `GOWORK=off`, and runs:

```sh
go test -mod=readonly -count=1 ./...
```

`-count=1` does not reuse a result cached from an in-checkout run. The copy
refuses symlinks, which could point back at the checkout. `--race` adds the
race detector.

`--self-test` builds two standard-library fixtures. One reads a sibling file
and still passes in place with `GOWORK=off` and a parent `go.work`. The gate
must fail that copy on `TestNeedsParent`. The other passes only when the test
process sees `GOWORK=off`.

The harness job runs the self-test, then the detached race suite, on Linux,
Windows, and macOS. `go vet ./...` and `go build ./cmd/pero-agent-acp` still
run in the checkout with `GOWORK=off`. The strict-sandbox and Windows ACL
jobs are unchanged: they require a real backend and still run inside the
checkout. This gate does not turn those into stand-ins. Unset
`PERO_REQUIRE_STRICT_SANDBOX` and `PERO_REQUIRE_WINDOWS_ACL` keep the
module's own skips.

## How to run

```sh
bash scripts/check/standalone-go-module.sh --self-test
bash scripts/check/standalone-go-module.sh
bash scripts/check/standalone-go-module.sh --race
```

## Local proof

Linux amd64. The default `go` was 1.22.2; `harness/go` selected Go 1.25.0.
`--self-test` passed. The default detached suite exited 0. Every package
printed `ok`:

`acp`, `agentinstructions`, `cmd/pero-agent-acp`, `config`, `fstools`,
`harness`, `internal/jsonwire`, `internal/sessionlogroute`, `llm`, `mcp`,
`media`, `model`, `nativetools`, `permissionpresets`, `providers/openai`,
`runtime`, `sandbox`, `session`, `sessionview`, `shell`, `skills`,
`storage`, `subagents`, `todos`, `tool`.

`bash scripts/check/standalone-go-module.sh --race` also exited 0 for the
same packages. `PERO_REQUIRE_STRICT_SANDBOX` and `PERO_REQUIRE_WINDOWS_ACL`
were unset, so neither run executed the positive bubblewrap or Windows ACL
contracts. In-memory MCP fixtures stayed fixtures.

## Still open

- `server`, including the root `ROADMAP.md` read and the real PostgreSQL suite.
- Every Cargo workspace member.
- npm packages have no module-local test command.
- Strict sandbox and Windows ACL jobs are real-backend gates inside the checkout, not detached copies.
