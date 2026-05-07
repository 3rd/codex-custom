# Custom Mod Manifest

This file is the behavior map for the `custom` branch. Use it with `git log main..custom`: the git range is the commit inventory, and this manifest is the intent that must survive upstream syncs.

Before adapting a mod, read the matching entry, inspect current upstream ownership, and re-derive the smallest implementation that preserves the behavior contract. Do not retire a mod without user confirmation.

## Sync Workflow And Local Build

- Mod ID: `sync-workflow`
- Current commits: `4876328d1`, current `chore: update skill` top commit
- Behavior contract: `main` mirrors `openai/main`, `custom` carries local patches, `make sync` syncs `main` then rebases `custom`, `make build` uses the local-release profile through the Nix shell, and `custom/run-tests.sh` runs the fork's full workspace validation without leaking local agent skills into app-server tests.
- Owner areas: `Makefile`, `codex-rs/Cargo.toml`, `flake.nix`, `custom/run-tests.sh`
- Targeted checks: `make sync-checklist`, `make -n sync-main`, `make -n rebase-custom`, `make -n sync`, `bash -n custom/run-tests.sh`
- Adaptation notes: preserve remote validation and the clean-worktree requirement before rebasing. Keep `main` disposable and never land custom work there. Keep `custom/run-tests.sh` aligned with upstream `just test`; it may bootstrap `cargo-nextest`, but it should not replace the upstream test path with raw `cargo test`.
- Retirement criteria: upstream provides an equivalent fork workflow or this checkout stops carrying local patches.

## MCP Approval Compatibility

- Mod ID: `mcp-approval-compat`
- Current commits: pending, updates `4876328d1`
- Behavior contract: non-`codex_apps` MCP tools in auto approval mode require approval under normal `on-request` policy when tool annotations are missing or all approval hints are unset. MCP tools with configured `approval_mode = "approve"` bypass MCP approval prompts and guardian/ARC review because the user has explicitly pre-approved that tool. In danger/full-access mode, MCP approval prompts auto-approve unless the tool is explicitly disabled or denied by config.
- Owner areas: `codex-rs/core/src/mcp_tool_call.rs`, `codex-rs/core/src/mcp_tool_call_tests.rs`, `codex-rs/core/src/guardian/mod.rs`, `codex-rs/core/src/lib.rs`, `codex-rs/codex-mcp/src/mcp/mod.rs`
- Targeted checks: `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core mcp_tool_call'`, `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-mcp mcp_prompt_auto_approval'`
- Adaptation notes: preserve explicit per-tool approval overrides for trusted local MCP servers that do not emit full annotations. Re-check upstream MCP approval semantics before replaying old code.
- Retirement criteria: upstream implements equivalent explicit per-tool approval overrides and danger/full-access MCP approval behavior.

## Danger Mode TUI

- Mod ID: `danger-mode-tui`
- Current commits: `be9e77869`
- Behavior contract: danger mode means every approval path is bypassed, including shell, patch, MCP, request-permissions, guardian/auto-review, and delegated subagent approvals. Only explicit configured deny rules remain hard blocks, including exec-policy forbidden rules, disabled MCP tools, denied network domains, and filesystem `none` deny entries/patterns.
- Owner areas: `codex-rs/protocol/src/models.rs`, `codex-rs/core/src/exec_policy.rs`, `codex-rs/core/src/state/turn.rs`, `codex-rs/core/src/session/mod.rs`, `codex-rs/core/src/codex_delegate.rs`, `codex-rs/core/src/tools/runtimes`, `codex-rs/tui/src`
- Targeted checks: `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core exec_policy'`, `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core mcp_tool_call'`, `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-tui'`, snapshot review if UI output changes
- Adaptation notes: treat state, rendering, tests, snapshots, and subagent/runtime permission inheritance as one bundle. Keep `Config` defaults and `TurnContext` runtime permissions aligned, and preserve explicit deny enforcement before bypassing approval prompts.
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

## Missing Project Metadata Bwrap Mounts

- Mod ID: `missing-project-metadata-bwrap-mounts`
- Current commits: pending
- Behavior contract: Linux bwrap sandbox setup must not create empty host-side `.agents` or `.codex` files when those protected project metadata roots are missing. Existing `.agents` and `.codex` directories remain protected.
- Owner areas: `codex-rs/linux-sandbox/src/bwrap.rs`
- Targeted checks: `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-linux-sandbox missing_project_metadata_roots_do_not_create_bwrap_mount_targets'`, `nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-linux-sandbox'`
- Adaptation notes: upstream issue `openai/codex#16088` and PR `openai/codex#16930` cover the broader `.codex` cleanup approach. Keep this fork patch narrow unless upstream lands a complete cleanup strategy that also covers `.agents`.
- Retirement criteria: upstream stops materializing missing protected project metadata mount targets for both `.codex` and `.agents`, or implements an equivalent cleanup strategy.

## Sync Upstream Agent Skill

- Mod ID: `sync-upstream-skill`
- Current commits: `6eb413d94`, `47c6f2458`, current `chore: update skill` top commit
- Behavior contract: coding agents have a repo-local skill that syncs from upstream, inventories fork-only commits, adapts mods one at a time, and reports sync status, mod decisions, pending confirmations, and verification.
- Owner areas: `.agents/skills/sync-upstream`, `AGENTS.md`, `CUSTOM_MODS.md`
- Targeted checks: read-through of the skill instructions against `AGENTS.md` and this manifest; `bash -n custom/run-tests.sh` when the full-test wrapper changes
- Adaptation notes: update this skill whenever sync ownership, verification commands, full-test wrapper behavior, or mod retirement rules change.
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
