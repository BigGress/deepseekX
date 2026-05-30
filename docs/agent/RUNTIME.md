# RUNTIME.md

## Purpose

This document defines how the in-app agent runtime is expected to behave.

It is written for both human contributors and coding agents working on the runtime.

## Main Entry Points

- Normal chat: `/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs` -> `send_message_via_api`
- Agent mode: `/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs` -> `run_agent_task`

Any behavior change that should apply to both modes must be verified against both paths.

## Runtime Components

### Planner

`planner.rs`

Responsibilities:

- build planner prompts
- parse LLM JSON action output
- normalize common schema drift

The planner must not directly execute actions. It only selects the next action.

### Orchestrator

`orchestrator.rs`

Responsibilities:

- hold the multi-step loop state
- call planner
- enforce step and timeout budgets
- invoke permission checks
- dispatch executor actions
- assemble the final response

### Executor

`executor.rs`

Responsibilities:

- read, write, rename, patch, and delete workspace files
- run guarded commands
- fetch URLs
- execute verification checks

### Permission Gate

`permissions.rs`

Responsibilities:

- classify action risk
- auto-allow safe actions
- block or require confirmation for risky actions
- enforce command allowlist behavior

## Goal Status Model

Supported terminal statuses:

- `done`
- `blocked`
- `needs_confirmation`

Interpretation:

- `done`: the runtime can provide a usable result now
- `blocked`: the runtime cannot make meaningful progress under current constraints
- `needs_confirmation`: a risky or externally consequential action must be approved first

## Observation Model

Each executed step should produce an observation containing:

- action name
- reason
- input summary
- result summary
- step status
- error or blocked state when applicable

Observations are the runtime’s authoritative execution history and are later used for:

- turn rendering
- retry and blocked summaries
- debugging
- persistence

## Approval Model

The default policy is:

- safe reads, safe retrieval, and allowlisted commands should execute directly
- destructive or clearly risky actions should require confirmation

Approval should be used sparingly. Requiring confirmation for ordinary read-only work is a product bug unless there is a concrete safety reason.

## Final Result Rules

Primary final results should contain:

- the answer or completion summary
- the verification summary
- optional next steps

Primary final results should not be overloaded with raw diagnostic detail such as:

- planner JSON internals
- long tool transcripts
- cited URL lists

That material belongs in execution detail or debug surfaces.
