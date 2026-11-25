# ADR-002 — Windows-native listener discovery

- **Status:** Accepted
- **Date:** 2026-08-09

## Context

PortSpy must show every local TCP listener on Windows with the owning PID,
including IPv4 and IPv6, and survive PID recycling. The obvious shortcuts —
parsing `netstat -ano` output or querying WMI in the hot refresh path — are
slow, brittle, or both.

## Decision

Use the Windows API directly via the `windows` crate, behind the
`Platform` trait:

- `GetExtendedTcpTable` (`TCP_TABLE_OWNER_PID_LISTENER`) for listener
  discovery — one call per IPv4/IPv6 family, no subprocess spawning.
- Toolhelp32 snapshot (`CreateToolhelp32Snapshot`) for bulk process metadata
  (PID, parent PID, name) — never per-listener queries.
- `OpenProcess` + `QueryFullProcessImageNameW` + `GetProcessTimes` +
  `GetProcessMemoryInfo` for per-process executable path, creation time, and
  working set.
- Command line via `NtQueryInformationProcess` + `ReadProcessMemory`, returning
  `None` gracefully when access is denied.
- Port numbers come in network byte order from the API and are converted
  explicitly.

The discovery pipeline runs in a background thread (see ADR-004); the UI never
blocks on OS calls. Metadata is collected in bulk and cached per refresh.

## Consequences

- A single snapshot needs no subprocess, no WMI, and no per-listener process
  queries — meeting the ~100-200 ms cold-snapshot target.
- Process identity is `PID + creation time`, defeating PID recycling.
- Permission failures degrade to partial information instead of errors:
  a process without command-line access is still shown with its name and path.

## Alternatives considered

- **`netstat -ano` parsing** — rejected: subprocess spawn per refresh, locale
  brittleness, no creation time.
- **WMI / CIM queries** — rejected: slow in the hot path, per-call overhead
  unacceptable at 2-second refresh cadence.
- **`windows-process-killer`-style crate** — rejected in favor of the explicit
  `windows` crate calls above for control over bulk collection.
