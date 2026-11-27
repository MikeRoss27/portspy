# ADR-006 Read-only MCP over stdio

**Status** Accepted

PortSpy can serve agent clients through `portspy mcp`. The CLI crate owns the
protocol and uses the existing snapshot builder over the injected platform.
There is no new service process beyond the client-launched executable. The
transport is standard input and output. Diagnostics go to stderr.

The server exposes local listener listing and exact port inspection. Inspection
returns every matching endpoint and observed process ancestry. MCP results use
the existing CLI JSON projection with bind scope added. They exclude command
lines and filesystem paths. The skill documents missing PID, multiple endpoint
matches and the distinction between inferred application labels and platform
facts.

Termination remains an explicit CLI or TUI operation. The MCP interface has no
tool for termination, shell execution or remote hosts.
