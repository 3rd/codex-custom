PREFIX ?= $(HOME)/.local
BIN_DIR ?= $(PREFIX)/bin
CARGO_HOME ?= /tmp/codex-custom-cargo-home
OPENAI_REMOTE ?= openai
OPENAI_URL ?= https://github.com/openai/codex.git
SYNC_BRANCH ?= main
CUSTOM_BRANCH ?= custom

.PHONY: require-custom build install ensure-openai sync sync-main rebase-custom update-custom

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

sync-main: ensure-openai
	@git fetch "$(OPENAI_REMOTE)" "$(SYNC_BRANCH)"
	@git fetch origin "$(SYNC_BRANCH)"
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
	@git branch --set-upstream-to="origin/$(SYNC_BRANCH)" "$(SYNC_BRANCH)" >/dev/null 2>&1 || true
	@git push origin "$(SYNC_BRANCH):$(SYNC_BRANCH)"

rebase-custom:
	@current_branch="$$(git branch --show-current)"; \
	if [ "$$current_branch" != "$(CUSTOM_BRANCH)" ]; then \
		printf 'check out %s before rebasing\n' "$(CUSTOM_BRANCH)" >&2; \
		exit 1; \
	fi; \
	stashed=0; \
	if [ -n "$$(git status --porcelain)" ]; then \
		git stash push --include-untracked -m "$(CUSTOM_BRANCH)-autostash-before-rebase" >/dev/null; \
		stashed=1; \
	fi; \
	if ! git rebase "$(SYNC_BRANCH)"; then \
		printf 'rebase failed; resolve conflicts and run git rebase --continue or git rebase --abort\n' >&2; \
		if [ "$$stashed" -eq 1 ]; then \
			printf 'autostash preserved in git stash list\n' >&2; \
		fi; \
		exit 1; \
	fi; \
	if [ "$$stashed" -eq 1 ]; then \
		git stash pop; \
	fi

update-custom:
	@$(MAKE) sync-main
	@$(MAKE) rebase-custom

sync: require-custom
	@$(MAKE) update-custom
