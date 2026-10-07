# Security Policy

## Supported versions

Security fixes are applied to the latest `main` branch and the most recent
tagged release when practical.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Prefer one of:

1. GitHub **Private vulnerability reporting** on this repository (Security tab), or
2. A private advisory / maintainer contact listed on the repository

Include:

- Affected version / commit
- Impact (e.g. arbitrary file read, local privilege escalation)
- Steps to reproduce
- Any suggested fix

You should receive an acknowledgment within a few days. We will coordinate a fix
and disclosure timeline.

## Scope notes

Race Refinery is a **local Windows desktop** app. It reads iRacing telemetry files and
shared memory, writes to `%LOCALAPPDATA%\race-refinery\`, and may install an
OpenXR API layer. There is **no cloud backend** and **no telemetry phone-home**.
See [docs/PRIVACY.md](docs/PRIVACY.md).
