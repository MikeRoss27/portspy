# ADR-001 — Workspace and crate boundaries

- **Status:** Accepted
- **Date:** 2026-08-09

## Context

PortSpy needs Windows OS integration, pure product logic, a terminal UI, and a
CLI entry point. Early on the natural trap is a single crate with a giant
`main.rs` and a catch-all `utils.rs`. We also want to keep `unsafe` code (Win32
calls) away from the rest of the codebase, and keep the TUI free of business
logic so that a future agent CLI or MCP server can reuse the same core.

## Decision

A Cargo workspace with four crates and strictly unidirectional dependencies:

```text
portspy-cli        composition, arguments, commands
  ├── portspy-tui  rendering, input, state (Ratatui only)
  └── portspy-core product logic, domain model — no OS, no UI
        └── portspy-platform  Win32 / unsafe — the only allowed boundary
```

- `portspy-core` defines a `Platform` trait; the real Windows implementation is
  injected from the CLI. Core never calls Win32 directly.
- `unsafe` is denied everywhere except `portspy-platform`.
- The platform crate stubs out on non-Windows targets
  (`UnsupportedPlatform`) so the workspace still builds and tests on Linux.
- Workspace-wide lints deny `unwrap`, `expect`, `todo!`, `panic`, `dbg!`,
  stdout/stderr printing, and missing docs on fallible APIs.

## Consequences

- Adding Linux/macOS support or an agent-facing mode later does not require
  rewriting the TUI or the detection engine.
- New crates are only created under real architectural pressure; the rule is
  "no unnecessary architecture".
- The strict lint policy makes panics in production code a compile-time error.

## Alternatives considered

- **Single crate with modules** — rejected: unsafe code would leak into the
  product layer; TUI and CLI could not be tested independently.
- **Hexagonal architecture with ports/adapters everywhere** — rejected as
  overkill: one clean `Platform` trait is enough.
