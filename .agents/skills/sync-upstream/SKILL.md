---
name: "sync-upstream"
description: "Use this skill when the user wants to sync this `codex-custom` fork with upstream, rebase `custom` onto the latest `openai/main`, port fork-only mods after upstream changes, or check whether upstream absorbed local patches. Use for requests like 'sync with upstream', 'refresh custom from upstream', 'rebase onto latest upstream', or 'carry our fork mods forward'. Do not use for generic git rebases, ordinary PR review, or standalone build/install work."
---

# Sync Upstream

## Task

$ARGUMENTS

## Expected Result

Produce or execute a fork refresh for this repo with:

- `sync status`: preflight state, `make sync` result, and any conflicts resolved
- `mod-by-mod decisions`: each fork-only commit in chronological order with `retain/port`, `retire`, or `blocked`
- `pending confirmations`: only decisions that need the user, especially retiring a mod because upstream absorbed it
- `verification`: targeted checks run per touched mod, plus any remaining gaps

## What This Repo Expects

- This checkout is a modded fork, not a generic `codex` clone.
- `main` is a disposable mirror of `openai/main`.
- `custom` is the only long-lived local patch branch.
- The repo-root `Makefile` is the source of truth for sync operations:
  - `make sync-main`
  - `make rebase-custom`
  - `make sync`
  - `make sync-checklist`
- Repo-root `CUSTOM_MODS.md` is the behavior manifest for fork-only mods.

## Process

### 1. Ground In Repo Truth

- Read the custom-fork sections in repo-root `AGENTS.md`, repo-root `CUSTOM_MODS.md`, and the sync targets in repo-root `Makefile`.
- Confirm branch and worktree state before doing anything:
  - `git status --short --branch`
  - `git remote -v`
- Run the non-mutating sync checklist:
  - `make sync-checklist`
- Enumerate fork-only commits in chronological order:
  - `git log --reverse --oneline main..custom`
- For each fork-only commit, inspect the scope before touching code:
  - `git show --stat --summary <sha>`
- Treat `git log main..custom` as the real commit inventory. Treat `CUSTOM_MODS.md` as the behavior intent. `AGENTS.md` may lag and should be updated if it no longer matches the active fork.

### 2. Run The Supported Sync Flow

- Work from `custom`.
- Start with a clean worktree; `make rebase-custom` refuses dirty tracked or untracked changes.
- Run `make sync`.
- If the repo is on the wrong branch, dirty in a way the workflow refuses, or the rebase fails, report the exact blocker first.
- When rebasing, resolve conflicts against current upstream behavior and the original purpose of the local mod. Do not replay old hunks blindly.
- Never land custom work on `main`.

### 3. Port Mods Sequentially

- Walk fork-only commits oldest to newest. Finish one mod before moving to the next.
- For each mod:
  - read the matching entry in `CUSTOM_MODS.md`
  - compare the original commit intent against the post-sync upstream tree
  - compare the manifest behavior contract against current upstream behavior
  - identify current owner files, tests, and snapshots
  - classify the mod as `retain/port`, `retire`, or `blocked`
- `retain/port`:
  - re-derive the mod in the smallest current owner location
  - preserve repo conventions and existing architecture
- `retire`:
  - only choose this when upstream already implements the behavior or the local divergence is no longer needed
  - show the evidence
  - ask the user for confirmation before removing or collapsing the mod
- `blocked`:
  - stop at the concrete blocker, state what was attempted, and leave the repo in a recoverable state
- If a mod's ownership, validation path, or maintenance notes change, update repo-root `AGENTS.md`.
- If a mod's behavior contract, owner areas, targeted checks, adaptation notes, or retirement status changes, update repo-root `CUSTOM_MODS.md`.
- Do not push `custom` automatically. Leave pushing as a separate, explicit step unless the user asks for it.

### 4. Verify Each Mod Before Continuing

- For the sync workflow and MCP approval patch in `codex-rs/core`:
  - run the smallest targeted `codex-core` test coverage for `mcp_tool_call`
- For Claude project doc fallbacks in `codex-rs/core`:
  - run the smallest targeted `codex-core` test coverage for `agents_md`
- For repo `.claude/skills` fallback in `codex-rs/core-skills`:
  - run targeted `codex-core-skills` loader coverage
- For the danger-mode TUI mod in `codex-rs/tui`:
  - run `cargo test -p codex-tui` from the `nix develop` shell path described in `AGENTS.md`
  - review and accept intended snapshot updates if the UI changed
  - then run `just fmt`
  - then run `just fix -p codex-tui`
- For other Rust edits, follow the crate-specific verification rules in repo-root `AGENTS.md`.
- Ask before running a full workspace validation sweep.
- When the user wants the full sweep, run the repo-root wrapper:
  - `./custom/run-tests.sh`
- Do not substitute raw `cargo test` for `./custom/run-tests.sh` or upstream `just test`. The wrapper preserves the upstream `cargo nextest` path, installs `cargo-nextest` if missing, and isolates `HOME` so local `$HOME/.agents/skills` does not change app-server skill tests.

## Output Rules

Always report the work in this order:

1. `sync status`
2. `mod-by-mod decisions`
3. `pending confirmations`
4. `verification`

Keep the write-up concise, but do not skip:

- which fork-only commit is being handled
- whether the mod was ported, retired pending confirmation, or blocked
- what evidence supports the decision
- what targeted verification ran

## Gotchas

- `make sync` is the supported entrypoint. Do not replace it with an improvised fetch/reset/rebase sequence.
- `make sync-main` validates both the upstream remote and the mirror remote before pushing `main`.
- `make rebase-custom` requires a clean worktree; stash or commit local work before syncing.
- Never retire a mod without user confirmation, even if upstream looks equivalent.
- `AGENTS.md` is required context, but it is not a complete fork inventory today. `git log main..custom` wins for commits; `CUSTOM_MODS.md` wins for behavior intent.
- TUI mods can span state, rendering, tests, and snapshots together. Treat them as one unit.
- Full-suite validation has local-environment traps. Use `./custom/run-tests.sh` so `$HOME/.agents/skills` does not leak into app-server tests, and so `RUST_MIN_STACK` plus nextest's app-server serialization are preserved.
- Runtime permission plumbing now has two layers: stored config defaults and active-turn runtime permissions. When adapting approval-review tests or `turn/contextUpdate`, update both layers when the test expects effective runtime behavior to change.
- If app-server warning tests report an unexpected omitted-skill count, suspect host skill discovery before changing assertions. The test process can see `$HOME/.agents/skills` unless the full-test wrapper isolates `HOME`.
- If shell snapshot runtime tests fail under raw `cargo test` with missing `/bin/bash`, do not patch the tests first; rerun through `./custom/run-tests.sh`/`just test` because the supported path is nextest in the Nix shell.

## Common Pitfalls

- Treating this as a generic upstream sync and forgetting that `main` is intentionally disposable here.
- Porting several mods together instead of verifying one mod at a time.
- Assuming a local mod still belongs in the same files after upstream refactors.
- Forgetting to update `AGENTS.md` after changing the owner files or retirement status of a fork-only mod.
- Pushing `custom` automatically after a rebase without the user asking for it.

## Checklist

- [ ] Preflight state captured from `custom`
- [ ] `CUSTOM_MODS.md` read and used as the behavior map
- [ ] `make sync-checklist` run before mutating sync commands
- [ ] Fork-only commit list derived from `git log main..custom`
- [ ] `make sync` used instead of a custom sync sequence
- [ ] Each mod handled in chronological order with `retain/port`, `retire`, or `blocked`
- [ ] Any retirement decision paused for user confirmation
- [ ] Targeted verification run for each touched mod
- [ ] Full validation, when requested, run via `./custom/run-tests.sh`
- [ ] Repo-root `CUSTOM_MODS.md` updated if behavior ownership or verification changed
- [ ] Repo-root `AGENTS.md` updated if the fork maintenance map changed
- [ ] No automatic push of `custom`
