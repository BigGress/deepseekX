# DEBUGGING.md

## Purpose

This document defines the default debugging workflow for runtime, planner, permission, and UI issues.

## Primary Evidence Sources

### LLM Request Logs

Path:

- `~/Library/Application Support/com.deepseekx.desktop/logs/llm-requests.jsonl`

Use this to inspect:

- request body
- raw response text
- request id
- duration
- status code
- success or failure

### Session Files

Path:

- `~/.deepseek/sessions/*.json`

Use these to inspect:

- persisted conversation history
- agent steps
- turn-level metadata
- debug responses attached to assistant messages

### App Settings

Stored through the settings table in the app database. Relevant keys include:

- `api_key`
- `knowledge_base_paths`
- `command_allowlist`
- `debug_llm_responses`

## Debug Mode

Enable debug mode from the app settings UI.

When enabled, the chat window shows a secondary debug panel with recent raw LLM responses. This is a diagnostic surface, not the primary result experience.

## Recommended Triage Order

### 1. Planner

Check:

- raw response text
- extracted JSON
- parser normalization behavior
- final parse error

Use this order whenever the UI shows:

- invalid planner json
- unsupported action
- unexpected schema drift

### 2. Permission Gate

Check:

- whether the action was inherently risky
- whether the command matched builtin allowlist rules
- whether the configured command allowlist should have allowed it

If a harmless read-only action still asks for confirmation, treat that as a product issue unless there is a concrete safety reason.

### 3. Executor

Check:

- workspace path resolution
- command spawn failures
- fetch failures
- file write and patch behavior

### 4. Turn Rendering

Check:

- session persistence
- turn grouping
- primary result content
- execution detail content
- debug panel rendering

## Common Debug Commands

```bash
tail -n 20 ~/Library/Application\\ Support/com.deepseekx.desktop/logs/llm-requests.jsonl
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri build
```

## Known Expectations

- Primary final results should not display cited URL lists.
- Low-risk actions should not require repeated approval.
- Configured command allowlist entries should be honored in both chat mode and agent mode.
- New runtime behavior should be explained by logs, steps, or persisted state, not guesswork.
