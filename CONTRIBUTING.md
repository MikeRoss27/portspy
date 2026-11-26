# Contributing to PortSpy

Thanks for contributing. PortSpy is a small, opinionated project: changes are
scoped, quality gates are strict, and the architecture is deliberate. Please
read `docs/architecture.md` and `AGENTS.md` before starting.

## Roadmap first

Check the roadmap in `README.md` before opening a PR. The v0.1 release gates
(Windows validation, CI, packaging) are the priority; v0.2+ features are listed
but should not be started before v0.1 ships.

## Ground rules

1. **Scoped changes only.** One PR = one logical change. No unrelated
   refactors, no arbitrarily "simplifying" another crate's architecture.
2. **Windows code inside the platform boundary.** `unsafe` lives only in
   `portspy-platform`.
3. **Business logic out of the TUI.** The TUI renders and dispatches actions;
   it never queries Win32.
4. **Tests with every change.** New detection rules need a positive and a
   negative test. State changes need `apply(Action)` tests.
5. **No hardcoded ports in tests** — use
   `TcpListener::bind("127.0.0.1:0")` and read the assigned port.
6. **Document boundary changes** in `docs/adr/` when you change a major
   architecture decision.

## Development workflow

1. Fork the repo, create a branch: `git checkout -b feature/your-change`.
2. Make your change with tests.
3. Run the complete quality gate:

   ```bash
   cargo fmt --all -- --check
   cargo check --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

   On Windows, `.\scripts\check.ps1` runs all of the above (plus
   `cargo deny check` when installed).

4. Push and open a pull request against `main`. Describe the scope, the tests
   added, and any docs updated.

## Reporting bugs

Bugs involving process detection should include:

```text
Port
Process name
PID
Command line if safe to share
Parent process chain
Windows version
Terminal / IDE / agent involved
```

**Do not** include secrets, tokens, environment variables, or private
command-line arguments (API keys, credentials, personal data) in public
issues.

## Security issues

See `SECURITY.md` — do not open a public issue for security problems.

## Code style

- Rust edition 2024, rustfmt defaults (see `rustfmt.toml`, `clippy.toml`).
- No `unwrap()` / `expect()` / `todo!()` / `panic!` in production code — the
  workspace lints enforce this.
- Errors are explicit (`anyhow` / `thiserror`); partial information is valid,
  missing metadata must never break the UI.
