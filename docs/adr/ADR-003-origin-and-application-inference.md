# ADR-003 — Origin and application inference

- **Status:** Accepted
- **Date:** 2026-08-09

## Context

The product promise is "`:3000 Next.js Codex 612 MB`" — understanding what a
process is *and where it came from*. Windows does not record "origin". We must
infer it from the process ancestry chain (parent, grandparent, ...) and the
command line, and present inferences honestly.

## Decision

Two independent detection layers, both declarative rule tables:

- **Application detection** (`detection/applications.rs`): what the process
  is — Next.js, Vite, Node, Python, PostgreSQL, Redis, Docker, Rust
  (`cargo run` / target paths).
- **Origin detection** (`detection/origins.rs`): where it came from — AI agent
  (Codex, Claude Code), IDE (Cursor, VS Code), terminal (Windows Terminal,
  PowerShell, CMD), Docker, WSL2.

Rules are plain data (process name, parent name, executable path, command-line
patterns) with a priority order: AI agent → IDE → terminal → environment.

Facts and inference are kept strictly separate:

- `ProcessInfo` carries only Windows facts (PID, name, path, command line,
  creation time, memory).
- `ApplicationInfo` / `OriginInfo` are inferred and always carry a
  `Confidence` (`Exact` / `High` / `Medium` / `Low` / `Unknown`).

Ancestry walks the parent chain depth-first, capped at 32 levels, cycle-safe,
tolerating garbage (disappeared processes, access denied).

## Consequences

- Adding a new agent (Gemini CLI, OpenCode, Aider, ...) is one declarative
  rule + one test — no engine surgery.
- The UI can show "origin unknown" without pretending; partial information is
  always valid.
- Every detection rule ships a positive and negative test (e.g.
  `git_bash_is_not_wsl`).

## Alternatives considered

- **Pattern matching on the whole ancestry as a single string** — rejected:
  brittle, no per-level attribution, hard to test.
- **Hardcoded if/else chains** — rejected: new agents/IDEs would require
  touching the engine.
- **Shell-parent "guessing" (e.g. conhost)** — rejected for v0.1; improved WSL2
  attribution is a tracked v0.2 item.
