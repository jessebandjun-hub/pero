#!/usr/bin/env bash
# Detach one Go module from this checkout and run its tests.
# GOWORK=off only disables Go workspace mode. A test can still open a parent
# file. The copy lives outside the repository, so that read fails.
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

usage() {
  cat >&2 <<'EOF'
usage: scripts/check/standalone-go-module.sh [--self-test] [--race] [MODULE_DIR]

With no arguments, copy harness/go outside this repository and run:
  GOWORK=off go test -mod=readonly -count=1 ./...
--race adds the race detector. --self-test checks the gate itself.
EOF
  exit 2
}

outside_repo() {
  local path=$1 repo=$2 phys repo_phys
  phys=$(cd "$path" && pwd -P)
  repo_phys=$(cd "$repo" && pwd -P)
  case "$phys" in
    "$repo_phys"|"$repo_phys"/*) return 1 ;;
  esac
}

copy_module() {
  local src=$1 dest=$2 rel dir
  if find "$src" \( -name 'bazel-*' \) -prune -o -type l -print -quit | grep -q .; then
    echo "standalone: $src contains a symlink; refusing to detach" >&2
    return 1
  fi
  mkdir -p "$dest"
  while IFS= read -r rel; do
    rel=${rel#./}
    dir=${rel%/*}
    if [[ $dir == "$rel" ]]; then
      dir=.
    fi
    mkdir -p "$dest/$dir"
    cp -p "$src/$rel" "$dest/$rel"
  done < <(cd "$src" && find . \( -name 'bazel-*' -o -name '*.test' \) -prune -o -type f -print)
  [[ -f $dest/go.mod ]]
}

run_detached() {
  local src=$1 repo=$2
  src=$(cd "$src" && pwd)
  [[ -f $src/go.mod ]] || { echo "standalone: not a Go module: $src" >&2; return 1; }
  (
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/pero-standalone.XXXXXX")
    trap 'rm -rf "$tmp"' EXIT
    outside_repo "$tmp" "$repo" || { echo "standalone: temp dir $tmp is inside the repository" >&2; exit 1; }
    dest="$tmp/$(basename "$src")"
    copy_module "$src" "$dest"
    outside_repo "$dest" "$repo" || { echo "standalone: copy is inside the repository" >&2; exit 1; }
    cd "$dest"
    export GOWORK=off
    work=$(go env GOWORK)
    [[ $work == off ]] || { echo "standalone: GOWORK=$work, want off" >&2; exit 1; }
    args=(-mod=readonly -count=1)
    if [[ ${RACE:-0} == 1 ]]; then
      args+=(-race)
    fi
    echo "standalone: GOWORK=off go test ${args[*]} ./... ($dest)"
    go test "${args[@]}" ./...
  )
}

self_test() {
  local repo tmp parent bad good output status
  repo=$(repo_root)
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/pero-standalone-self.XXXXXX")
  # Expand now. A RETURN trap runs after locals are unset, and set -u would fail.
  trap "rm -rf $(printf '%q' "$tmp")" RETURN
  parent=$tmp/parent
  bad=$parent/bad
  good=$tmp/good
  mkdir -p "$bad" "$good"
  printf 'marker\n' >"$parent/SECRET"
  cat >"$parent/go.work" <<'EOF'
go 1.22

use ./bad
EOF
  cat >"$bad/go.mod" <<'EOF'
module example.com/bad

go 1.22
EOF
  cat >"$bad/escape_test.go" <<'EOF'
package bad

import (
	"os"
	"testing"
)

func TestNeedsParent(t *testing.T) {
	if _, err := os.ReadFile("../SECRET"); err != nil {
		t.Fatal(err)
	}
}
EOF
  cat >"$good/go.mod" <<'EOF'
module example.com/good

go 1.22
EOF
  cat >"$good/alone_test.go" <<'EOF'
package good

import (
	"os"
	"testing"
)

func TestWorkspaceModeOff(t *testing.T) {
	if os.Getenv("GOWORK") != "off" {
		t.Fatalf("GOWORK=%q", os.Getenv("GOWORK"))
	}
}
EOF
  # Workspace mode is off and the parent file is still there. The test passes.
  (cd "$bad" && GOWORK=off go test -mod=readonly -count=1 .)
  set +e
  output=$(RACE=0 run_detached "$bad" "$repo" 2>&1)
  status=$?
  set -e
  if [[ $status -eq 0 ]]; then
    echo "standalone: self-test accepted a module that reads a parent file" >&2
    return 1
  fi
  if ! grep -q 'TestNeedsParent' <<<"$output"; then
    echo "standalone: self-test failed before the parent-file test:" >&2
    printf '%s\n' "$output" >&2
    return 1
  fi
  echo "standalone: self-test rejected the parent-file module"
  RACE=0 run_detached "$good" "$repo"
  echo "standalone: self-test passed"
}

main() {
  local self=0 module="" repo
  RACE=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --self-test) self=1 ;;
      --race) RACE=1 ;;
      -h|--help) usage ;;
      -*) usage ;;
      *)
        [[ -z $module ]] || usage
        module=$1
        ;;
    esac
    shift
  done
  repo=$(repo_root)
  if [[ $self == 1 ]]; then
    [[ -z $module ]] || usage
    self_test
    return
  fi
  if [[ -z $module ]]; then
    module=$repo/harness/go
  elif [[ $module != /* ]]; then
    module=$repo/$module
  fi
  run_detached "$module" "$repo"
  echo "standalone: ${module#"$repo"/} tests passed outside the repository"
}

main "$@"
