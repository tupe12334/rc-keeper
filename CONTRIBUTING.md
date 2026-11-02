# Contributing to rc-keeper

Thanks for helping. This page covers development. For users, see the [README](README.md).

## Prerequisites

- Rust stable, with `rustfmt` and `clippy`.
- Node 22 and pnpm 10.
- Tauri 2 system dependencies for your OS. See the [Tauri prerequisites](https://tauri.app/start/prerequisites/).
- Claude Code CLI, for manual testing.

## Repo layout

The module contract is in [CONTRACT.md](CONTRACT.md). It defines each folder and its public API. Read it before you change a folder's interface.

## Run

Run the UI alone:

```sh
pnpm -C ui dev
```

Run the desktop app:

```sh
cargo tauri dev
```

Run the command-line tool:

```sh
cargo run -p rc-cli -- status --json
```

## Tests and lints

CI runs the checks in [`.github/workflows/ci.yml`](.github/workflows/ci.yml) on every push and pull request. Run the same commands locally before you open a pull request.

Tests must use a temporary config folder. Never test against your real home folder or your real rc-keeper config.

### Headless Linux check

Run [`scripts/headless-linux-check.sh`](scripts/headless-linux-check.sh) when you change the CLI, the daemon or the service code. It needs Docker. The script builds the CLI in a container, then runs it in a clean `ubuntu:24.04` with no GUI and no systemd, as a non-root user, with a fake `claude`. This is how rc-keeper runs on a server over SSH. The script checks the full flow: add, status, stop and start, service install, remove and daemon stop. It prints `FAIL: <reason>` and exits 1 on the first problem.

## API docs

The CLI and IPC reference in [`docs/reference`](docs/reference) is generated from the code. So is `ui/src/api/generated.ts`, the UI's IPC types. Do not edit these files by hand.

```sh
pnpm -C ui install                  # once: the TypeScript step needs ui/node_modules
cargo run -p xtask -- docs          # regenerate
scripts/check-generated-docs.sh     # fails when the files are stale
```

Change the clap definitions or the IPC types, then regenerate and commit the output. The rustdoc for `rc-core` is generated too: `cargo doc -p rc-core --no-deps`.

## Commits and pull requests

- Use conventional commit style for the PR title, for example `feat: add folder flags` or `fix: restart after exit`.
- We squash merge. The PR title becomes the commit message.
- release-please reads these titles to choose the version and write the changelog.

## Releases

release-please opens a release PR on `main`. A maintainer merges that PR. The merge creates the tag and the GitHub release. The workflow then builds and smoke-tests the bundles.

See [`.github/workflows/release-please.yml`](.github/workflows/release-please.yml).

## Code rules

- One responsibility per file.
- File names describe the job of the file.
- Prefer maintained libraries over hand-written code.
- Every change ships with tests.

## Docs rules

- Each fact lives in one place. Link to it instead of copying it.
- Link to code instead of copying facts from it.
- Draw diagrams in Mermaid, not as image files.
