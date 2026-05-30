# CONTRIBUTING.md

## Purpose

This document defines the default development workflow for `DeepSeekX`.

Use it when modifying:

- React UI in `/Users/Gress/code/ai/deepseekX/src`
- Tauri command handlers in `/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs`
- Agent runtime code in `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent`
- Persistent app behavior such as sessions, settings, logs, or project metadata

## Local Development

Install dependencies and use the project from the repository root.

```bash
npm install
```

Common commands:

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

## Change Workflow

1. Inspect the current implementation before editing.
2. Identify whether the change affects:
   - frontend rendering
   - tauri command wiring
   - agent runtime behavior
   - persistence or debugging surfaces
3. Make the smallest change that satisfies the requirement.
4. Run the relevant verification commands.
5. Update stable docs when behavior, workflow, or runtime expectations changed.

## Verification Expectations

Minimum expectations:

- UI-only presentational changes: `npm run build`
- Rust logic or agent behavior changes: `cargo test --manifest-path src-tauri/Cargo.toml`
- Bundle, desktop startup, or packaging changes: `npm run tauri build`

When a change crosses layers, run all relevant checks.

## Frontend Guidelines

- Keep the chat window usable in both normal mode and debug mode.
- Prefer shared `Turn`-based rendering instead of reviving legacy message-only branches.
- If a change affects agent output display, inspect:
  - primary result
  - execution detail
  - artifact panels
  - debug panel

## Agent Runtime Guidelines

- Do not change planner prompts without checking parser compatibility.
- Do not add new tool behavior without updating permission evaluation and executor handling together.
- Do not expose citations or diagnostic material in the primary result unless that is explicitly intended product behavior.

## Persistence Guidelines

When changing any of the following, confirm both read and write paths:

- settings
- projects
- sessions
- turn grouping
- debug logs

## Documentation Expectations

Update docs when any of these change:

- runtime flow
- permission model
- command allowlist behavior
- debug mode behavior
- required verification steps

Use:

- `/Users/Gress/code/ai/deepseekX/docs/development` for contributor-facing guidance
- `/Users/Gress/code/ai/deepseekX/docs/agent` for runtime and harness-facing guidance
- `/Users/Gress/code/ai/deepseekX/docs/superpowers` for dated design and plan documents
