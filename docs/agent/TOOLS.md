# TOOLS.md

## Purpose

This document describes the tool and permission model used by the in-app runtime.

## Supported Action Families

### Retrieval

- `retrieve_context`

Used to gather information from enabled sources such as:

- workspace code
- workspace docs
- user knowledge base
- web search

### File Reads and Writes

- `read_files`
- `create_files`
- `rename_files`
- `delete_files`
- `apply_patch`
- `write_files`

Default preference:

- use `apply_patch` for targeted edits
- use `create_files` for new files
- use `write_files` only for full replacement cases

### Commands and Verification

- `run_command`
- `verify_checks`

These are guarded by:

- workspace path checks
- shell syntax restrictions
- builtin allowlist rules
- configured command allowlist entries

### Network Reads

- `fetch_url`

Use this for external read-only HTTP access instead of pushing external `curl` or `wget` into `run_command`.

### MCP

- `call_mcp_tool`

Only valid when MCP tools have actually been discovered for the current runtime.

### Reporting and Control

- `summarize_findings`
- `ask_user`
- `finish`

## Permission Model

### Auto-Allowed by Default

Typical low-risk actions should run without confirmation:

- retrieval
- file reads
- safe file creation and patching
- safe rename operations
- ordinary `fetch_url`
- allowlisted `run_command`
- allowlisted `verify_checks`

### Requires Confirmation

High-risk or irreversible actions still require confirmation:

- file deletion
- dangerous shell commands
- commands outside the builtin or configured allowlist
- clearly oversized write operations that exceed normal editing expectations

### Command Allowlist

There are two allowlist layers:

1. builtin safe commands in `permissions.rs`
2. user-configured command prefixes from app settings

Configured allowlist behavior:

- one command prefix per line
- prefix match is token-based
- matching commands can run directly
- compound shell syntax is still rejected

Examples:

- `npm run build`
- `pnpm run dev`
- `python -m pytest`

## Retrieval Source Expectations

The runtime must only use enabled retrieval sources for the active project.

Do not assume web search or knowledge base access is available unless the current project enables it.

## Product Rule

Tool outputs can appear in execution detail and debug panels, but the primary user result should stay concise and product-facing.
