# Custom Mod Manifest

This file is the behavior map for the `custom` branch. Use it with `git log main..custom`: the git range is the commit inventory, and this manifest is the intent that must survive upstream syncs.

Before adapting a mod, read the matching entry, inspect current upstream ownership, and re-derive the smallest implementation that preserves the behavior contract. Do not retire a mod without user confirmation.

## Sync Workflow And Local Build

- Mod ID: `sync-workflow`
- Current commits: `4876328d1`, current `chore: update skill` top commit
- Behavior contract: `main` mirrors `openai/main`, `custom` carries local patches, `make sync` syncs `main` then rebases `custom`, and `make build` uses the local-release profile through the Nix shell.
- Owner areas: `Makefile`, `codex-rs/Cargo.toml`, `flake.nix`
- Targeted checks: `make sync-checklist`, `make -n sync-main`, `make -n rebase-custom`, `make -n sync`
- Adaptation notes: preserve remote validation and the clean-worktree requirement before rebasing. Keep `main` disposable and never land custom work there.
- Retirement criteria: upstream provides an equivalent fork workflow or this checkout stops carrying local patches.

## MCP Approval Compatibility

- Mod ID: `mcp-approval-compat`
- Current commits: `4876328d1`
- Behavior contract: non-`codex_apps` MCP tools in auto approval mode skip the default prompt when tool annotations are missing or all approval hints are unset; if approval is still reached while approvals are never allowed, decline instead of prompting.
- Owner areas: `codex-rs/core/src/mcp_tool_call.rs`
- Targeted checks: `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core mcp_tool_call'`
- Adaptation notes: preserve compatibility with local MCP servers that do not emit full annotations. Re-check upstream MCP approval semantics before replaying old code.
- Retirement criteria: upstream implements equivalent behavior for unannotated local MCP tools.

## Danger Mode TUI

- Mod ID: `danger-mode-tui`
- Current commits: `be9e77869`
- Behavior contract: the TUI exposes and reflects the local danger-mode permission state consistently between runtime behavior and visible UI.
- Owner areas: `codex-rs/tui/src`
- Targeted checks: `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-tui'`, snapshot review if UI output changes
- Adaptation notes: treat state, rendering, tests, and snapshots as one bundle. Prefer TUI-local ownership unless the current upstream architecture requires a cross-process concept.
- Retirement criteria: upstream exposes equivalent permission behavior and visible state.

## Claude Project Doc Fallback

- Mod ID: `claude-project-doc-fallback`
- Current commits: `b2936c7e4`
- Behavior contract: built-in project-doc discovery recognizes `CLAUDE.md` and `.claude/CLAUDE.md` when `AGENTS.md` is absent, while `AGENTS.md` remains preferred per directory.
- Owner areas: `codex-rs/core/src/agents_md.rs`
- Targeted checks: `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core agents_md'`
- Adaptation notes: preserve the one-doc-per-directory rule when upstream changes discovery order or file naming.
- Retirement criteria: upstream implements equivalent fallback discovery.

## Claude Skill Root Fallback

- Mod ID: `claude-skill-root-fallback`
- Current commits: `f2545ab6b`
- Behavior contract: repo-local `.agents/skills` is preferred, and repo-local `.claude/skills` is used only when `.agents/skills` is absent.
- Owner areas: `codex-rs/core-skills/src/loader.rs`
- Targeted checks: `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core-skills loader'`
- Adaptation notes: keep `.agents/skills` as the primary project skill directory. Do not merge both roots unless the intended behavior changes.
- Retirement criteria: upstream implements equivalent fallback skill-root loading.

## Sync Upstream Agent Skill

- Mod ID: `sync-upstream-skill`
- Current commits: `6eb413d94`, `47c6f2458`, current `chore: update skill` top commit
- Behavior contract: coding agents have a repo-local skill that syncs from upstream, inventories fork-only commits, adapts mods one at a time, and reports sync status, mod decisions, pending confirmations, and verification.
- Owner areas: `.agents/skills/sync-upstream`, `AGENTS.md`, `CUSTOM_MODS.md`
- Targeted checks: read-through of the skill instructions against `AGENTS.md` and this manifest
- Adaptation notes: update this skill whenever sync ownership, verification commands, or mod retirement rules change.
- Retirement criteria: upstream or the active agent runtime provides an equivalent fork-sync workflow.

## RTK Shell Approval Normalization

- Mod ID: `rtk-shell-approval-normalization`
- Current commits: `357144ac8`
- Behavior contract: leading `rtk` wrapper tokens are stripped before shell approval heuristics and exec policy matching, so commands such as `rtk ls -la` are evaluated like `ls -la`.
- Owner areas: `codex-rs/shell-command/src`, `codex-rs/core/src/exec_policy.rs`
- Targeted checks: focused shell-command safety coverage and `codex-core` exec policy coverage
- Adaptation notes: preserve safety classification after unwrapping. Check direct safe, dangerous, and exec policy paths.
- Retirement criteria: upstream supports wrapper-token normalization or the local workflow no longer requires `rtk`.

## Live Turn Context Updates

- Mod ID: `live-turn-context-updates`
- Current commits: `c5c6badbb`, `d42e832cb`
- Behavior contract: `turn/contextUpdate` updates the active turn and subsequent defaults without new user input, and pending interactive requests are re-evaluated when runtime permissions change.
- Owner areas: `codex-rs/app-server-protocol`, `codex-rs/app-server`, `codex-rs/core`, `codex-rs/tui`
- Targeted checks: app-server v2 turn interrupt coverage, core session coverage, TUI permissions coverage, schema regeneration when protocol shapes change
- Adaptation notes: treat protocol, app-server plumbing, core turn state, TUI request resolution, tests, generated schemas, and README examples as one behavior bundle. Do not split the runtime behavior from the visible permission/request state.
- Retirement criteria: upstream implements equivalent live context-update semantics and pending-request re-evaluation.
