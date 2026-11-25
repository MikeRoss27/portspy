# ADR-005 — Native Linux and macOS backends

- **Status:** Accepted
- **Date:** 2026-11-25

PortSpy needs the same socket, process and termination operations on Windows,
Linux and macOS. Every backend implements the existing `Platform` trait
inside `portspy-platform`. The CLI selects a backend at compile time and the
snapshot and detection code consume the same raw types on all three systems.

Linux reads the kernel's `/proc/net` tables through `procfs` in the current
network namespace. It joins socket inodes to visible process file descriptors
and reports sockets with no accessible owner using PID 0. macOS uses
`libproc` to read socket descriptors directly from accessible processes.
The two backends share process metadata collection via `sysinfo` and signal
delivery via `rustix`. TCP discovery includes listening sockets and UDP
discovery includes bound endpoints. Permissions and process exits can limit
the information returned by either backend.

The native CI matrix includes Windows, Linux and macOS. Platform tests bind
ephemeral TCP and UDP sockets and check ownership and address conversion.
