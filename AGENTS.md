# AGENTS.md — Guide for AI coding agents

This file gives AI agents (Codex, Claude Code, opencode, Cursor, ...) the
minimum context needed to work safely on PortSpy. Read the full architecture
notes in `docs/architecture.md` (design rationale) and `docs/devplan.md`
(working plan) before making structural changes.

## Project

PortSpy — "Know what's using your localhost." A Windows-first Rust TUI that
lists local TCP listeners, identifies the owning process, reconstructs its
origin (parent chain → agent/IDE/terminal), and kills it from a clean UI.

- Target: **Windows 11**, `x86_64-pc-windows-msvc`, Rust edition 2024, MSRV
  1.85 (pinned via `rust-toolchain.toml`).
- Current state: v0.1 feature set implemented; remaining gates are Windows
  validation, CI (see `.github/workflows/ci.yml`), and packaging.

## Architecture

Cargo workspace, 4 crates, strictly unidirectional dependencies:

```text
portspy-cli        composition, args, commands (main.rs, args.rs, output.rs)
  ├── portspy-tui  Ratatui UI — rendering, input, state (app/state/input/ui)
  └── portspy-core product logic — domain, snapshot, detection, processes
        └── portspy-platform  Win32 / unsafe — the ONLY allowed boundary
```

- `portspy-core` depends on a `Platform` trait; the CLI injects the real
  `WindowsPlatform`. Core never calls Win32.
- Facts (Windows metadata) and inference (application/origin detection) are
  separate types; every inference carries a `Confidence`.
- No `utils.rs` / `helpers.rs` / `common.rs` catch-alls. New crates only under
  real architectural pressure.
- No database, no daemon, no remote API in v0.1.

## Quality gates — MUST pass before any commit

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check          # optional, supply chain
```

Windows cross-check (works from WSL/Linux):

```bash
cargo check --workspace --target x86_64-pc-windows-msvc
```

## Rules

1. **No `unwrap()` / `expect()` / `todo!()` / `panic!` / `dbg!()` in
   production code** — the workspace lints deny them. Handle errors explicitly
   (`anyhow` / `thiserror`) and degrade to partial information.
2. **`unsafe` only inside `portspy-platform`.** Do not widen the unsafe
   boundary.
3. **No unrelated refactors** — a change must not arbitrarily "simplify"
   another crate. No scope creep.
4. **Add tests with every change** — new detection rules need a positive and a
   negative test; state changes need `apply(Action)` tests with the
   `FakePlatform`.
5. **Never hardcode ports in tests** — bind `TcpListener::bind("127.0.0.1:0")`
   and read the assigned port. No magic ports like 3000.
6. **Windows code stays in the platform crate; business logic stays out of the
   TUI.**
7. **`missing_errors_doc` and `missing_panics_doc` are denied** — document
   fallible APIs.
8. **Update `docs/adr/` when changing a major architectural boundary.**
9. **Don't commit secrets, tokens, or env var values** — bug reports must be
   scrubbed of private command-line arguments.

## Useful reference points

- CLI contract: `crates/portspy-cli/src/output.rs` (stable JSON schema)
- Detection rule tables: `crates/portspy-core/src/detection/`
- Snapshot pipeline: `crates/portspy-core/src/snapshot/`
- Platform trait: `crates/portspy-core/src/platform.rs` (trait) +
  `crates/portspy-platform/src/` (Windows impl)
- TUI state machine: `crates/portspy-tui/src/state.rs` + `input/`
- Roadmap: `README.md` (v0.2: UDP, tree-kill, WSL attribution, duplicate
  detection) — check it before starting new work.
