# AGENTS.md

This repository contains `DeepSeekX`, a Tauri desktop application with an in-app chat runtime and an agent runtime.

This file is the required entrypoint for both human contributors and coding agents. Read it first, then follow the linked documents for detail.

## Project Scope

- Frontend: `/Users/Gress/code/ai/deepseekX/src`
- Tauri commands and app bridge: `/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs`
- Agent runtime: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent`
- Desktop bundle config: `/Users/Gress/code/ai/deepseekX/src-tauri/tauri.conf.json`
- Product and implementation docs: `/Users/Gress/code/ai/deepseekX/docs`

## Hard Rules

- Keep changes aligned with the actual runtime behavior. Do not document, claim, or expose capabilities that are not wired into code.
- Prefer small, direct changes that preserve existing patterns in the Tauri, React, and agent runtime layers.
- Do not silently weaken high-risk safety gates such as file deletion, irreversible writes, or external side effects.
- When changing agent behavior, inspect both execution paths:
  - `run_agent_task`
  - `send_message_via_api`
- When changing tool permissions or command execution, update both:
  - permission evaluation
  - command execution
- When changing session, turn, or debug output behavior, verify both persisted data and rendered UI surfaces.

## Required Verification

Run the smallest relevant verification for the files you touched. For behavioral changes, prefer full checks.

- Frontend changes: `npm run build`
- Rust / Tauri / agent runtime changes: `cargo test --manifest-path src-tauri/Cargo.toml`
- Release bundle or desktop runtime changes: `npm run tauri build`

Do not claim a fix is complete unless the relevant verification actually passed.

## Working Conventions

- Keep user-facing copy in Simplified Chinese unless the file already follows another convention.
- Use repository-relative paths in runtime data and file operations; keep workspace boundaries explicit.
- Treat logs, session files, and debug output as diagnostic surfaces, not the primary user experience.
- New specs and plans belong under `/Users/Gress/code/ai/deepseekX/docs/superpowers`.
- Stable contributor and runtime guidance belongs in the documents below, not in dated plan files.

## Document Map

- Development workflow: `/Users/Gress/code/ai/deepseekX/docs/development/CONTRIBUTING.md`
- System architecture: `/Users/Gress/code/ai/deepseekX/docs/development/ARCHITECTURE.md`
- Agent runtime model: `/Users/Gress/code/ai/deepseekX/docs/agent/RUNTIME.md`
- Tooling and permission model: `/Users/Gress/code/ai/deepseekX/docs/agent/TOOLS.md`
- Debugging and log inspection: `/Users/Gress/code/ai/deepseekX/docs/agent/DEBUGGING.md`

## When In Doubt

- If you are touching user interaction, inspect the current UI before changing copy or layout assumptions.
- If you are touching planner, permissions, retrieval, or executor logic, capture the raw runtime evidence first.
- If a change belongs to both human workflow and agent runtime behavior, update both the development docs and the agent docs in the same change.

@RTK.md
