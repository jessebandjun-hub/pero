# Architecture constraints

ROADMAP asks for a quality gate that fails on an illegal dependency, protocol
drift, a missing single state owner, or a source file past the length ceiling.
This record is the part that is enforced today. The ROADMAP item stays open:
protocol samples and a single state owner are not new checks here.

## Enforced

Two harness packages already state their imports. Production Go files (not
`_test.go`) must match those lists. The standard library is allowed. A module
path whose first element contains a dot is not the standard library.

| Package | Allowed beyond the standard library | Source |
| --- | --- | --- |
| `harness/go/llm` | `github.com/phodal/pero/harness/go/model` | `harness/go/llm/README.md` |
| `harness/go/session` | `model`, `runtime`, and `golang.org/x/sys/...` | `harness/go/session/README.md` |

`harness/go/go.mod` must not contain a `replace` directive. `model/README.md`
and `session/README.md` both say the module has no local replace.

A code file stays within 1000 lines, tests included. The walk covers `.go`,
`.rs`, `.ts`, `.tsx`, `.js`, `.mjs`, and `.py`, including `protocol/generated`.
It skips dependency, build, and VCS directories. HTML prototypes are outside
this ceiling. No file is grandfathered: nothing in the tree is over the limit,
and this gate does not add `scripts/file-lines-baseline.json`.

The same allow function rejects a fixture that imports `session` from `llm`.
The same line counter rejects a 1001-line buffer. The checkout itself is the
passing case.

## Already enforced elsewhere

| Rule | Where |
| --- | --- |
| `model` production sources import only the standard library | `harness/go/model/dependencies_test.go` |
| `runtime` production sources import only the standard library | `harness/go/runtime/boundary_test.go` |
| Host and remote protocol samples match the TypeSpec generators | `bazel test //:checks` and `npm run check` |

## Not this gate

- One state owner per transition. `session` is documented as the event-log
  owner. That is not an import or line-count check.
- Control-plane package layering inside `server/`. The architecture note names
  process owners, not a Go import table.
- `AGENTS.md` points at `backend/architecture_test.go`. That tree is not in
  this repository. New harness rows belong in
  `harness/go/architecture/imports_test.go`.

## Run

From a full checkout, this package walks the repository and is the file-length
gate. The harness CI job runs it via `go test ./...`. Bazel's harness module
does not contain the rest of the checkout, so its copy checks imports and
`go.mod` only.

```sh
cd harness/go
GOWORK=off go test -count=1 ./architecture
GOWORK=off go test -race -count=1 ./architecture
```

The standalone Bazel module runs the same package:

```sh
cd harness/go
bazel test //architecture:architecture_test
```
