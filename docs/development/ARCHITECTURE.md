# ARCHITECTURE.md

## Overview

`DeepSeekX` is a desktop application built with:

- React + TypeScript for the UI
- Tauri for the desktop shell and command bridge
- A Rust application runtime that implements chat, agent orchestration, retrieval, permissions, and persistence

The product supports two main execution modes:

- normal chat
- agent task execution

## Top-Level Structure

### Frontend

`/Users/Gress/code/ai/deepseekX/src`

Key areas:

- app shell and state: `App.tsx`
- chat layout: `components/ChatPanel.tsx`
- turn rendering: `components/TurnItem.tsx` and `components/turn/*`
- project and settings UI: `components/ProjectSelector.tsx`, `ProjectSettings.tsx`, `SettingsDialog.tsx`
- tauri invoke wrappers: `api.ts`

### Tauri and App Bridge

`/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs`

This file is the main command surface between the UI and the Rust runtime. It contains:

- project CRUD commands
- settings commands
- session loading commands
- chat execution entrypoint: `send_message_via_api`
- agent execution entrypoint: `run_agent_task`
- log-reading and debug support

### Agent Runtime

`/Users/Gress/code/ai/deepseekX/src-tauri/src/agent`

Important modules:

- `planner.rs`: planner prompt construction and JSON action parsing
- `orchestrator.rs`: multi-step agent loop and final response assembly
- `executor.rs`: file, command, and fetch execution
- `permissions.rs`: permission gate and command allowlist evaluation
- `retrieval.rs`: workspace, knowledge base, and web retrieval
- `mcp.rs`: MCP tool discovery and invocation
- `state.rs`: observation, step status, and runtime state types
- `report.rs`: summarized reporting helpers

## Runtime Paths

### Normal Chat

UI -> `send_message_via_api` -> chat tool loop -> optional tool actions -> final assistant response

This path supports tool execution, but is budgeted as a shorter loop than full agent mode.

### Agent Mode

UI -> `run_agent_task` -> planner -> permission gate -> executor -> observation loop -> finish

This path is the primary multi-step runtime and is where approval, retries, and structured observations are centered.

## Persistence

### SQLite

`/Users/Gress/code/ai/deepseekX/src-tauri/src/db.rs`

Stores:

- projects
- conversations
- messages
- app settings

### Session Files

Stored under:

- `~/.deepseek/sessions/*.json`

Used for turn grouping, chat history, and agent step reconstruction.

### LLM Request Logs

Stored under:

- `~/Library/Application Support/com.deepseekx.desktop/logs/llm-requests.jsonl`

Used for debugging raw request and response behavior.

## Presentation Model

The chat window is organized around `Turn` objects rather than plain message bubbles.

Main display containers:

- user intent
- primary result
- execution detail
- artifacts
- state and actions

This separation matters because agent results often include:

- final natural-language answers
- structured execution steps
- diffs or file previews
- approval states
- debug-only runtime evidence

## Safety Boundaries

Core safety boundaries currently live in:

- workspace path resolution
- permission evaluation
- command allowlist handling
- retrieval source restrictions

Any change that weakens these areas must be deliberate and documented.
