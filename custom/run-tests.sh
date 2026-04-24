#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
real_home="${HOME:-}"
test_home="${CODEX_CUSTOM_TEST_HOME:-}"

if [[ -z "$test_home" ]]; then
  test_home="$(mktemp -d "${TMPDIR:-/tmp}/codex-custom-test-home.XXXXXX")"
  cleanup_test_home=1
else
  mkdir -p "$test_home"
  cleanup_test_home=0
fi

if [[ "$cleanup_test_home" == 1 ]]; then
  trap 'rm -rf "$test_home"' EXIT
fi

export HOME="$test_home"

if [[ -n "${CARGO_HOME:-}" ]]; then
  export CARGO_HOME
elif [[ -n "$real_home" ]]; then
  export CARGO_HOME="$real_home/.cargo"
fi

if [[ -n "${CARGO_HOME:-}" ]]; then
  export PATH="$CARGO_HOME/bin:$PATH"
fi

cd "$repo_root"
exec nix develop . --command bash -lc '
  set -euo pipefail
  if ! cargo nextest --version >/dev/null 2>&1; then
    echo "cargo-nextest is missing; installing with cargo install --locked cargo-nextest" >&2
    cargo install --locked cargo-nextest
  fi
  exec just test
'
