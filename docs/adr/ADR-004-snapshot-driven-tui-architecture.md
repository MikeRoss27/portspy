# ADR-004 — Snapshot-driven TUI architecture

- **Status:** Accepted
- **Date:** 2026-08-09

## Context

The TUI refreshes itself every 2 seconds while the user may be mid-search,
mid-scroll, or confirming a kill. Naive designs either block the UI on OS calls
or rebuild UI state from scratch every tick, losing the selection and the
query. A 100+ listener machine must stay responsive.

## Decision

Three pillars:

1. **Immutable system snapshot.** `SnapshotBuilder` produces a
   `SystemSnapshot` through a fixed pipeline (listeners → bulk process
   metadata → ancestry → application detection → origin detection). Snapshots
   are immutable and diffable (`SnapshotDiff`: appeared / disappeared /
   changed).
2. **Background collector thread.** A collector thread builds snapshots every
   2 seconds and sends them over an `mpsc` channel. The UI thread never calls
   the OS.
3. **Stable identity + action-driven state.** `AppState` holds
   `selected: Option<ListenerId>` — never a `usize` index. `ListenerId`
   (protocol + address + port + PID + start time) survives refreshes, so
   selection stays stable across snapshot replacements. All mutations go
   through `apply(Action)`, which makes the state machine testable without a
   terminal.

After confirmation, a worker collects a fresh snapshot, compares it with the
reviewed scope, and stops the reviewed processes through the `Platform` trait
when the scope matches. The UI can continue handling input and rendering while
the worker runs. The worker returns the outcome through the message channel
for display in the footer.

## Consequences

- Refresh cost is invisible to the user: render is bounded by the frame, not
  by the scan.
- Selection, query, and sort survive auto-refresh.
- The TUI holds zero Win32 knowledge — business logic stays in `portspy-core`,
  and the event/state layer is fully unit-testable (32 tests).

## Alternatives considered

- **Synchronous scan in the event loop** — rejected: UI freezes on large
  snapshots and on slow metadata collection.
- **Index-based selection** — rejected: rows reorder between refreshes; a
  selected index can silently point at the wrong process.
- **Shared-mutable-state with locks** — rejected in favor of the immutable
  snapshot + channel pattern, which makes races structurally impossible.
