# PortSpy

> Know what's using your localhost.

**PortSpy** is a fast, Windows-first terminal UI for discovering local listening ports, understanding what is behind them, identifying where processes came from, and stopping processes you no longer need.

Instead of this:

```text
TCP    127.0.0.1:3000    LISTENING    18432
```

PortSpy aims to show you this:

```text
:3000    Next.js        Codex           612 MB       12m
:3001    Next.js        Claude Code     584 MB        8m
:5173    Vite           Cursor          321 MB         1h
:5432    PostgreSQL     Docker          148 MB         4h
```

No hunting through Task Manager.
No `netstat` + PID lookup + process-tree archaeology.
No wondering which AI coding agent started yet another dev server.

<p align="center">
  <a href="https://portspy.dev">portspy.dev</a>
</p>

---

## Status

> [!NOTE]
> The full `v0.1` feature set is implemented: 93 tests green (core + TUI), `cargo clippy --all-targets` clean. The remaining release gates are validation on a real Windows machine (listener discovery, PID resolution, tool detection) and packaging — tracked in `docs/devplan.md`.

`v0.1` does one thing and does it well:

**discover local TCP listeners → identify their process and origin → inspect them → kill them from a clean TUI.**

Windows 11 is the primary target for the first release.

---

## Why PortSpy?

Modern development environments create a surprising amount of background state.

A single project can involve:

- Codex
- Claude Code
- Cursor
- VS Code
- Windows Terminal
- PowerShell
- WSL2
- Docker
- Next.js
- Vite
- local databases
- development servers
- workers
- debuggers

And every one of them can start processes.

Eventually you end up asking:

```text
What is using port 3000?

Why is there something on 3001?

Did Codex start this?

Is this Next.js instance still useful?

Can I safely kill it?
```

Windows can already give you most of the raw information.

The problem is that the information is fragmented.

PortSpy turns it into something useful.

---

## The idea

PortSpy does not treat a listening socket as just:

```text
PID 18432
```

It reconstructs the context around it.

For example:

```text
WindowsTerminal.exe
└── pwsh.exe
    └── codex.exe
        └── node.exe
            └── localhost:3000
```

PortSpy can then present that as:

```text
Port         :3000
Application  Next.js
Origin       Codex
Runtime      Node.js
PID          18432
Memory       612 MB
Uptime       12m
```

The raw process information remains available when you want it.

The useful answer comes first.

---

## TUI

The interface is intentionally minimal.

```text
 portspy  BROWSE  8 listening · 12 total              2.1 GB · 12m

 PORT     APP       ORIGIN       PID     PROCESS      MEM     UPTIME
 :3000    Next.js   Codex        18432   node.exe     612 MB  12m
 :3001    Next.js   Claude Code  20144   node.exe     584 MB   8m
 :5173    Vite      Cursor       5120    node.exe     321 MB   1h
 :5432    PostgreSQL Docker      9812    postgres.exe 148 MB  4h
 :8000    Python    PowerShell   9021    python.exe   96 MB   5m


 ↑/↓ move · j/k vim · Enter inspect · / search · s sort · S order · x kill · q quit
```

Inspecting an entry opens the same list on the left and a details split on the right:

```text
 PORT     APP       ORIGIN       PID     │ Next.js
 :3000    Next.js   Codex        18432   │
 :3001    Next.js   Claude Code  20144   │ listener  :3000 (tcp)
 :5173    Vite      Cursor       5120    │ address   127.0.0.1:3000
 :5432    PostgreSQL Docker      9812    │ process   node.exe
 :8000    Python    PowerShell   9021    │ pid       18432
                                         │ memory    612 MB
                                         │ uptime    12m
                                         │ command   pnpm dev
                                         │ cwd       C:\dev\my-app
                                         │
                                         │ ├── node.exe (pid 18432)
                                         │ ├── codex.exe (pid 15320)
                                         │ └── pwsh.exe (pid 9124)
```

The interface is designed around:

- low visual noise
- fast keyboard navigation
- stable selection during refreshes
- responsive terminal layouts
- useful information before implementation details
- no unnecessary dashboards
- no rainbow CLI
- no Matrix aesthetic

---

## Core features

### Port discovery

PortSpy discovers local listening TCP sockets and associates them with their owning processes.

The first release targets:

- IPv4
- IPv6
- loopback listeners
- all-interface listeners
- interface-specific listeners

PortSpy distinguishes between addresses such as:

```text
127.0.0.1
::1
0.0.0.0
::
```

so that localhost-only listeners are not presented the same way as processes exposed on every interface.

### Process inspection

For every listener, PortSpy attempts to retrieve:

- PID
- process name
- executable path
- command line
- parent PID
- process creation time
- memory usage
- CPU usage
- process ancestry
- working directory when available

Not every piece of metadata is always accessible on Windows.

Missing information is treated as missing information, not as a fatal error.

### Process ancestry

PortSpy reconstructs the parent process chain.

Example:

```text
Windows Terminal
└── PowerShell
    └── Claude Code
        └── Node.js
```

This is what allows PortSpy to answer a much more useful question than:

> Which executable owns this socket?

It can attempt to answer:

> What actually caused this process to exist?

### Origin detection

The first release is designed to recognize origins such as:

- Codex
- Claude Code
- Cursor
- VS Code
- Windows Terminal
- PowerShell
- CMD
- WSL2
- Docker

PortSpy keeps factual process information separate from inferred information.

For example:

```text
Fact:
node.exe owns PID 18432

Inference:
the application appears to be Next.js

Inference:
the process appears to originate from Codex
```

Detection rules carry confidence information internally rather than pretending every inference is certain.

### Application detection

PortSpy tries to show the application you care about instead of only the runtime executable.

Instead of:

```text
node.exe
```

it can identify:

```text
Next.js
```

or:

```text
Vite
```

Initial detection targets include:

- Next.js
- Vite
- Node.js
- Python
- Rust applications
- PostgreSQL
- Docker-related processes

When a framework cannot be identified reliably, PortSpy falls back to the runtime or raw executable.

### Kill processes

Processes can be stopped directly from the TUI.

```text
Kill Next.js :3000 (PID 18432)?

y confirm · n cancel · Esc cancel
```

`x` requests the kill on the selected listener; confirmation is always explicit.

PortSpy does not silently kill processes merely because they look unnecessary.

Destructive actions require explicit user intent.

Elevated or protected Windows processes refuse termination at the operating-system level; PortSpy reports the failure cleanly as an access error instead of crashing.

### Search

Search across listener metadata:

```text
/
```

Searchable fields include:

- port
- application
- process
- origin
- PID
- command line

### Sorting

Listeners can be sorted by:

- port
- application
- origin
- process
- memory
- uptime

`s` cycles through the fields, `S` flips the direction. Sorting is stable: listeners sharing a sort key keep their snapshot order.

### Auto refresh

A background collector thread rebuilds the system snapshot every 2 seconds and hands it to the UI through a channel, so terminal interaction is never blocked by discovery.

The selected listener is tracked using a stable listener identity rather than a table index.

That means a disappearing process should not make the cursor randomly jump around the interface.

---

## CLI

Running PortSpy without a subcommand launches the TUI:

```bash
portspy
```

The core is independent from the TUI so that the same engine also powers scriptable commands:

```bash
portspy list                 # table of local listeners
portspy list --json          # machine-readable output
portspy inspect 3000         # details for the process behind :3000
portspy inspect 3000 --json
portspy kill 3000            # confirmation prompt (--yes to skip)
```

Example `list --json` output:

```json
{
  "listeners": [
    {
      "port": 3000,
      "protocol": "tcp",
      "address": "127.0.0.1",
      "application": { "kind": "nextjs", "confidence": "high" },
      "process": {
        "pid": 18432,
        "name": "node.exe",
        "memory_bytes": 641728512,
        "started_at": { "secs_since_epoch": 1754678400, "nanos_since_epoch": 0 }
      },
      "origin": { "kind": "codex", "confidence": "high" }
    }
  ]
}
```

The JSON interface and TUI use the same core domain model.

---

## Architecture

PortSpy is written in Rust.

The repository is intentionally split into a small number of focused crates:

```text
portspy/
├── crates/
│   ├── portspy-cli/
│   ├── portspy-core/
│   ├── portspy-platform/
│   └── portspy-tui/
│
├── tests/
├── docs/
├── scripts/
└── .github/
```

### `portspy-core`

The product logic.

Responsibilities include:

- domain models
- snapshot construction
- process ancestry
- application detection
- origin detection
- listener enrichment
- filtering and sorting primitives

The core contains no TUI code and should not depend directly on Windows UI concerns.

### `portspy-platform`

Operating-system integration.

The Windows implementation is responsible for things such as:

- TCP listener discovery
- PID ownership
- process metadata
- process creation times
- parent processes
- memory information
- process termination

Windows-native APIs are isolated here.

Any necessary `unsafe` code must remain inside the platform boundary.

### `portspy-tui`

The terminal interface.

Built around Ratatui.

Responsibilities include:

- rendering
- keyboard input
- application state
- selection
- responsive layouts
- details view
- search UI
- confirmation state
- notifications

The TUI does not query Win32 directly.

### `portspy-cli`

The executable entry point.

It composes:

```text
platform
   ↓
core
   ↓
TUI / CLI output
```

It also owns command-line argument parsing and non-interactive commands.

---

## Data flow

PortSpy builds immutable snapshots of the current machine state.

```text
Windows
   │
   ▼
listener discovery
   │
   ▼
unique process IDs
   │
   ▼
process metadata
   │
   ▼
process ancestry
   │
   ▼
application detection
   │
   ▼
origin detection
   │
   ▼
SystemSnapshot
   │
   ├── TUI
   └── CLI / JSON
```

Expensive system discovery is never performed directly inside the rendering path.

A background collector produces snapshots while the TUI remains responsive.

---

## Design principles

### Facts and inference are separate

Windows process metadata is factual.

Application and origin detection are inference.

PortSpy does not pretend otherwise.

### Partial information is valid

Processes can disappear during collection.

Permissions can deny metadata access.

PIDs can be recycled.

Command lines may be unavailable.

PortSpy must still produce the best snapshot possible.

One inaccessible process must never break the entire interface.

### No giant `main.rs`

The repository should remain understandable as it grows.

Responsibilities belong in explicit modules.

Avoid catch-all files such as:

```text
utils.rs
helpers.rs
misc.rs
common.rs
```

unless they represent a genuinely coherent abstraction.

### No UI business logic

The rendering layer renders state.

It does not discover ports.

It does not inspect processes.

It does not infer Codex.

It does not decide how Windows processes should be killed.

### No unnecessary architecture

PortSpy starts with four crates.

A new crate should only exist when there is a real architectural boundary that justifies it.

---

## Windows implementation

PortSpy is Windows-first.

TCP listener discovery is designed around native Windows networking APIs such as:

```text
GetExtendedTcpTable
```

Process metadata is collected through appropriate Win32 APIs.

The first target is:

```text
Windows 11
x86_64-pc-windows-msvc
```

WSL2 awareness will progressively improve after the initial Windows foundation is stable.

---

## Safety

PortSpy can terminate local processes, so destructive behavior is deliberately conservative.

Principles:

- killing requires explicit confirmation
- process disappearance is handled safely
- permission failures do not crash PortSpy
- critical Windows processes are protected
- no automatic duplicate termination
- no background daemon in `v0.1`
- no remote API in `v0.1`

PortSpy is a local developer tool, not a process supervisor.

At least not yet.

---

## Development

### Requirements

- Windows 11
- Rust stable
- Visual Studio Build Tools / MSVC toolchain
- Git

Verify:

```bash
rustc --version
cargo --version
```

Clone the repository:

```bash
git clone https://github.com/<org-or-user>/portspy.git
cd portspy
```

Build:

```bash
cargo build --workspace
```

Run:

```bash
cargo run -p portspy-cli
```

Release build:

```bash
cargo build --release
```

---

## Quality gates

Every change should pass:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Additional dependency and supply-chain checks may use:

```bash
cargo deny check
```

Production code should avoid:

```rust
unwrap()
expect()
todo!()
dbg!()
```

unless there is a deliberate and documented reason.

Unsafe code belongs only in the platform implementation where native Windows APIs require it.

---

## Testing strategy

PortSpy uses several layers of testing.

### Unit tests

Used for deterministic logic such as:

- origin detection
- application detection
- sorting
- filtering
- listener identity
- process ancestry reconstruction

Example:

```text
WindowsTerminal.exe
└── pwsh.exe
    └── codex.exe
        └── node.exe
```

must resolve to:

```text
Origin: Codex
```

### Fixture tests

Synthetic process trees allow detection logic to be tested without depending on the developer's machine.

### Windows integration tests

Integration tests can create real temporary listeners:

```rust
TcpListener::bind("127.0.0.1:0")
```

The operating system assigns an available port.

PortSpy must then discover that listener and associate it with the expected process.

Tests should never depend on hardcoded development ports such as `3000`.

### Race-condition testing

System inspection is inherently racy.

Tests must account for processes that:

- appear
- disappear
- restart
- change state
- become inaccessible

while a snapshot is being created.

PortSpy should degrade gracefully rather than panic.

---

## Roadmap

### `v0.1`

The first usable release.

- [x] Windows TCP listener discovery
- [x] IPv4
- [x] IPv6
- [x] PID resolution
- [x] process metadata
- [x] process creation time
- [x] process ancestry
- [x] application detection
- [x] origin detection
- [x] Codex detection
- [x] Claude Code detection
- [x] Cursor detection
- [x] VS Code detection
- [x] Windows Terminal detection
- [x] PowerShell detection
- [x] WSL awareness
- [x] responsive Ratatui interface
- [x] listener list
- [x] inspect view
- [x] search
- [x] sorting
- [x] stable auto-refresh
- [x] process kill
- [x] kill confirmation
- [x] JSON output
- [ ] Windows CI
- [ ] standalone release binary

### `v0.2`

Deeper process intelligence.

- [ ] UDP listeners
- [ ] process-tree termination
- [ ] improved WSL2 attribution
- [ ] project detection
- [ ] working-directory detection improvements
- [ ] duplicate dev-server detection
- [ ] orphan process detection
- [ ] improved runtime detection

### `v0.3`

Development environment awareness.

- [ ] Docker awareness
- [ ] resource history
- [ ] CPU trends
- [ ] memory trends
- [ ] service grouping
- [ ] project grouping

### `v0.4`

Agent integration.

- [ ] stable agent-facing CLI
- [ ] Codex integration
- [ ] Claude Code integration
- [ ] agent skills
- [ ] MCP server
- [ ] shared runtime awareness

### `v1.0`

Long-term direction.

- [ ] stable CLI contract
- [ ] stable JSON schema
- [ ] Windows production support
- [ ] Linux support
- [ ] macOS support
- [ ] extensible detection rules
- [ ] mature WSL2 support

---

## What PortSpy is not

PortSpy `v0.1` is deliberately **not**:

- a port scanner
- a network pentesting tool
- a firewall manager
- a Docker manager
- a system monitor replacement
- a process supervisor
- a daemon
- a cloud service
- an AI wrapper

It is a focused local developer tool.

```text
Find the port.
Understand the process.
Know where it came from.
Stop it when you want.
```

---

## Contributing

PortSpy is still early, so architectural consistency matters more than raw feature count.

Before contributing:

1. keep changes scoped
2. do not mix unrelated refactors with features
3. add tests for new detection rules
4. keep Windows-specific code inside the platform boundary
5. keep business logic out of the TUI
6. update architecture documentation when changing a major boundary
7. run the complete quality gate before submitting a pull request

Bug reports involving process detection should ideally include:

```text
Port
Process name
PID
Command line if safe to share
Parent process chain
Windows version
Terminal / IDE / agent involved
```

Do not include secrets, tokens, environment variables, or private command-line arguments in public issues.

---

## Architecture decisions

Significant architectural decisions are documented under:

```text
docs/adr/
```

Initial decisions include:

```text
ADR-001  Workspace and crate boundaries
ADR-002  Windows-native listener discovery
ADR-003  Origin and application inference
ADR-004  Snapshot-driven TUI architecture
```

ADRs exist for decisions that would be expensive to reverse later.

---

## Security

If you discover a security issue in PortSpy, please do not disclose it publicly before maintainers have had a reasonable opportunity to investigate.

See:

```text
SECURITY.md
```

for the project's disclosure process.

---

## License

License information will be available in:

```text
LICENSE
```

---

<p align="center">
  <strong>PortSpy</strong><br>
  Know what's using your localhost.<br><br>
  <a href="https://portspy.dev">portspy.dev</a>
</p>
```
