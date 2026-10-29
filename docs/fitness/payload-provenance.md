---
kind: verification_report
date: 2026-10-29
---

# Declared payload provenance

ROADMAP 「构建与发布溯源」 stays unchecked. Acceptance is that packaged files
match a declared payload and that the artifact digest traces to a source
revision and dependency locks. This note records an L0 gate for one package.
It does not mark the item complete.

## What is checked

`@pero/app-sdk` is the first declared payload
([packages/app-sdk/payload.json](../../packages/app-sdk/payload.json)).
`npm run test:payload` compares that manifest to the package directory and to
the file members of an `npm pack` tarball. A missing file, an extra file, or a
symlink fails the check. The command prints a provenance receipt:

- `sourceRevision` is `git rev-parse HEAD`.
- `dependencies` records the sha256 of each declared lock. This payload cites
  the repository `package-lock.json`.
- `payloadSha256` is the content digest of the declared files.
- `archiveSha256` is the sha256 of the tarball produced by that run.

The check refuses a dirty payload directory or lock path, so it does not issue
a receipt for a working tree that differs from HEAD.

`payloadSha256` is sha256 over the declared paths in order. Each entry is the
UTF-8 path, a NUL byte, the hex sha256 of that file, and a newline. The rule
lives in `DeclaredPayload.receipt`.

## How to run

```sh
npm run test:payload
```

CI runs that command on Linux, Windows, and macOS in the desktop job, before
the native build. It needs Node.js and npm. It does not need `npm ci`, Rust,
or a desktop package.

## Remaining gaps

The native release archives (Pero on macOS, Windows, and Linux, and Virtual
Cell) still stage directory trees. Publication already records a source
revision and an archive sha256 in `.build.json`, and a denylist rejects a few
private paths. Those archives do not yet have an exact declared file list, and
their build metadata does not record `Cargo.lock` or `package-lock.json`
digests. `Cargo.lock`, `server/go.sum`, and `harness/go/go.sum` are not
materials of the App SDK payload.

The App SDK archive vendors no third-party code. `package-lock.json` is the
repository npm lock observed at pack time; the SDK is not an npm workspace
member. React peers are not installed into the tarball. Tarball bytes can
differ across npm versions because archive metadata is not canonical.
`payloadSha256` is the content digest.
