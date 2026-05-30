# Tooling Backlog

Standalone backlog for missing runtime tools and capability gaps in DeepSeekX. This file is intentionally separate from historical implementation plans.

## Tool Runtime

- [x] Add a real MCP tool execution layer so selected project MCP servers are discoverable and callable at runtime, not only listed in the system prompt.
- [x] Add a real Skill invocation path so selected project skills affect execution behavior, not only prompt context injection.
- [x] Make slash commands executable: `/skill xxx` and `/mcp xxx` should trigger explicit app behavior instead of acting as autocomplete-only text insertion.

## Approval Loop

- [x] Add a user approval flow for `ask_user` / `needs_confirmation`, including approve, reject, and retry actions in the UI.
- [x] Persist enough pending-agent state to resume execution after approval instead of forcing the user to restate the task.

## Retrieval Sources

- [x] Implement a real `web_search` provider for `retrieve_context` instead of returning `unavailable`.
- [x] Implement a real `user_knowledge_base` source for `retrieve_context` instead of returning `unavailable`.
- [x] Add project-level controls for retrieval sources so web-enabled and local-only projects can be configured explicitly.

## Editing Tools

- [x] Add a structured patch-edit action such as `apply_patch` to avoid whole-file overwrite for small changes.
- [x] Add explicit file operations for rename, delete, and create so the planner does not need to model everything as `write_files`.
- [x] Return richer structured write results for file edits, including operation type (`create`, `modify`, `overwrite`, `delete`) and changed ranges where possible.

## Command And Verification Tools

- [x] Expand the guarded command allowlist for common engineering workflows such as `pytest`, `uv`, `node`, `make`, and more granular git inspection commands.
- [x] Separate read-only external fetch commands from side-effecting commands so network lookups can be approved and explained more clearly.
- [x] Add a higher-level verification action that can summarize test/build results instead of forcing every verification step through raw shell output.

## Chat Mode Parity

- [x] Add a true tool-calling path for normal chat mode so it is not limited to plain model output plus `file:` block auto-write.
- [x] Unify chat-mode and agent-mode execution reporting so both modes can show structured tool steps when tools are used.

## Browser And Desktop Tools

- [x] Add browser automation capability for tasks that need page navigation, scraping, or UI validation.
- [x] Add desktop/computer-use capability for tasks that require native app interaction beyond the workspace file system.
