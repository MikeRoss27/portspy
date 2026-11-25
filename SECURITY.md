# Security Policy

## Reporting a vulnerability

If you discover a security issue in PortSpy, please **do not open a public
issue** before maintainers have had a reasonable opportunity to investigate
(typically 7 days for confirmed issues, longer for complex ones).

Contact the maintainers privately with the following information:

- Affected version / commit
- Steps to reproduce
- Impact assessment (what an attacker could gain)
- Any proof-of-concept, scrubbed of sensitive data

## Scope

PortSpy is a local, stateless CLI/TUI tool. It does not:

- open network connections beyond local listener enumeration (read-only OS
  APIs)
- ship a daemon, server, or remote API
- collect or transmit telemetry
- store data persistently

The kill feature (`portspy kill <port>` / `x` in the TUI) terminates the
owning process and requires explicit confirmation. It refuses known protected
processes.

## Non-goals / out of scope

- Malicious use of the kill feature by another local user is not a PortSpy
  vulnerability (equivalent privileges would allow `taskkill`).
- False positives in origin/application detection are product bugs, not
  security issues — report them as regular issues with the process context
  (see CONTRIBUTING.md).

## Process

1. Reporter contacts maintainers privately.
2. Maintainer confirms the issue and its severity.
3. A fix is developed and released.
4. The issue is disclosed publicly after release (or earlier by agreement).

## Data handling

Do **not** include secrets, tokens, environment variables, or private
command-line arguments (API keys, credentials, personal data) in any report.
