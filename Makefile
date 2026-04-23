PREFIX ?= $(HOME)/.local
BIN_DIR ?= $(PREFIX)/bin
CARGO_HOME ?= /tmp/codex-custom-cargo-home
OPENAI_REMOTE ?= openai
OPENAI_URL ?= https://github.com/openai/codex.git
MIRROR_REMOTE ?= origin
MIRROR_URL ?= github_3rd:3rd/codex-custom
SYNC_BRANCH ?= main
CUSTOM_BRANCH ?= custom

.PHONY: require-custom build clean install ensure-openai ensure-mirror sync sync-main rebase-custom update-custom sync-checklist

require-custom:
	@current_branch="$$(git branch --show-current)"; \
	if [ "$$current_branch" != "$(CUSTOM_BRANCH)" ]; then \
		printf 'check out %s first\n' "$(CUSTOM_BRANCH)" >&2; \
		exit 1; \
	fi

build: require-custom
	mkdir -p "$(CARGO_HOME)"
	nix develop . --command bash -lc 'export CARGO_HOME="$(CARGO_HOME)"; cd codex-rs && cargo build -p codex-cli --bin codex --profile local-release'

install: require-custom build
	mkdir -p "$(BIN_DIR)"
	install -m755 ./codex-rs/target/local-release/codex "$(BIN_DIR)/codex"

ensure-openai:
	@if git remote get-url "$(OPENAI_REMOTE)" >/dev/null 2>&1; then \
		current_url="$$(git remote get-url "$(OPENAI_REMOTE)")"; \
		if [ "$$current_url" != "$(OPENAI_URL)" ]; then \
			printf 'refusing to overwrite remote %s (found %s)\n' "$(OPENAI_REMOTE)" "$$current_url" >&2; \
			exit 1; \
		fi; \
	else \
		git remote add "$(OPENAI_REMOTE)" "$(OPENAI_URL)"; \
	fi

ensure-mirror:
	@if ! git remote get-url "$(MIRROR_REMOTE)" >/dev/null 2>&1; then \
		printf 'missing mirror remote %s\n' "$(MIRROR_REMOTE)" >&2; \
		exit 1; \
	fi
	@fetch_url="$$(git remote get-url "$(MIRROR_REMOTE)")"; \
	push_url="$$(git remote get-url --push "$(MIRROR_REMOTE)")"; \
	if [ "$$fetch_url" != "$(MIRROR_URL)" ]; then \
		printf 'refusing to use mirror remote %s fetch URL %s; expected %s\n' "$(MIRROR_REMOTE)" "$$fetch_url" "$(MIRROR_URL)" >&2; \
		exit 1; \
	fi; \
	if [ "$$push_url" != "$(MIRROR_URL)" ]; then \
		printf 'refusing to use mirror remote %s push URL %s; expected %s\n' "$(MIRROR_REMOTE)" "$$push_url" "$(MIRROR_URL)" >&2; \
		exit 1; \
	fi

sync-main: ensure-openai ensure-mirror
	@git fetch "$(OPENAI_REMOTE)" "$(SYNC_BRANCH)"
	@git fetch "$(MIRROR_REMOTE)" "$(SYNC_BRANCH)"
	@if ! git show-ref --verify --quiet "refs/heads/$(SYNC_BRANCH)"; then \
		printf 'missing local branch %s\n' "$(SYNC_BRANCH)" >&2; \
		exit 1; \
	fi
	@if ! git merge-base --is-ancestor "$(SYNC_BRANCH)" "$(OPENAI_REMOTE)/$(SYNC_BRANCH)"; then \
		printf 'local %s has commits not present in %s/%s; refusing to move it\n' "$(SYNC_BRANCH)" "$(OPENAI_REMOTE)" "$(SYNC_BRANCH)" >&2; \
		exit 1; \
	fi
	@current_branch="$$(git branch --show-current)"; \
	if [ "$$current_branch" = "$(SYNC_BRANCH)" ]; then \
		if [ -n "$$(git status --porcelain)" ]; then \
			printf '%s is checked out and dirty; commit or stash first\n' "$(SYNC_BRANCH)" >&2; \
			exit 1; \
		fi; \
		git merge --ff-only "$(OPENAI_REMOTE)/$(SYNC_BRANCH)"; \
	else \
		git branch -f "$(SYNC_BRANCH)" "$(OPENAI_REMOTE)/$(SYNC_BRANCH)"; \
	fi
	@git branch --set-upstream-to="$(MIRROR_REMOTE)/$(SYNC_BRANCH)" "$(SYNC_BRANCH)" >/dev/null 2>&1 || true
	@git push "$(MIRROR_REMOTE)" "$(SYNC_BRANCH):$(SYNC_BRANCH)"

rebase-custom:
	@current_branch="$$(git branch --show-current)"; \
	if [ "$$current_branch" != "$(CUSTOM_BRANCH)" ]; then \
		printf 'check out %s before rebasing\n' "$(CUSTOM_BRANCH)" >&2; \
		exit 1; \
	fi; \
	if [ -n "$$(git status --porcelain)" ]; then \
		printf '%s is dirty; commit or stash before rebasing\n' "$(CUSTOM_BRANCH)" >&2; \
		exit 1; \
	fi; \
	if ! git rebase "$(SYNC_BRANCH)"; then \
		printf 'rebase failed; resolve conflicts and run git rebase --continue or git rebase --abort\n' >&2; \
		exit 1; \
	fi

update-custom:
	@$(MAKE) sync-main
	@$(MAKE) rebase-custom

sync: require-custom
	@$(MAKE) update-custom

sync-checklist:
	@printf 'branch state:\n'
	@git status --short --branch
	@printf '\nremotes:\n'
	@git remote -v
	@printf '\nfork-only commits:\n'
	@git log --reverse --oneline "$(SYNC_BRANCH)..$(CUSTOM_BRANCH)"
	@printf '\nstandard verification commands:\n'
	@printf "  nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core mcp_tool_call'\n"
	@printf "  nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core agents_md'\n"
	@printf "  nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-core-skills loader'\n"
	@printf "  nix develop . --command bash -lc 'cd codex-rs && cargo test -p codex-tui'\n"
	@printf "  nix develop . --command bash -lc 'cd codex-rs && just fmt'\n"
	@printf "  nix develop . --command bash -lc 'cd codex-rs && just fix -p codex-tui'\n"
