# ADR-005 — Native Linux and macOS backends

- **Status:** Accepted
- **Date:** 2026-11-25

PortSpy exposes socket, process and termination operations through the same
interface on Windows, Linux and macOS. Every backend implements the `Platform`
trait inside `portspy-platform`. The CLI selects a backend at compile time.
Snapshot and detection code consume the same raw types on all three systems.

Linux reads the kernel's `/proc/net` tables through `procfs` in the current
network namespace. It joins socket inodes to visible process file descriptors
and reports sockets with no accessible owner using PID 0. Unattributed sockets
can still carry their kernel-reported UID and service unit. Linux queries
socket diagnostics for the cgroup v2 ID and matches the reply by socket inode,
protocol and endpoint. The service unit is resolved through the cgroup v2
filesystem. Process termination still requires a verified PID. macOS uses
`libproc` to read socket descriptors directly from accessible processes.
Linux reads metadata from open `/proc` process directories. It uses a PID
file descriptor to send signals to a verified process. macOS uses `sysinfo`
for process metadata and reports that reviewed termination is unsupported.
TCP discovery includes listening sockets and UDP discovery includes bound
endpoints. Permissions and process exits can limit the information returned
by either backend.

The native CI matrix includes Windows, Linux and macOS. Platform tests bind
ephemeral TCP and UDP sockets and check ownership and address conversion.
