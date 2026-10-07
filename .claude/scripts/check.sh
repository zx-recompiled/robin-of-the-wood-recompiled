#!/usr/bin/env bash
# check.sh — the pre-PR gate. Exits non-zero if anything fails.
#
# Runs what CI runs, and then the differential suites, which CI cannot: they
# need the player's own copy of the game and the Spectrum ROM, and those are
# never committed. So this script is the only place the whole gate exists.
#
# Gate on the EXIT CODE. Never grep the output.
set -uo pipefail

cd "$(dirname "$0")/../.." || exit 1
failed=()
run() {
  local name="$1"; shift
  echo "=== $name"
  if "$@"; then return 0; fi
  failed+=("$name")
}

run "fmt"          cargo fmt --all --check
run "build"        cargo build --workspace --all-targets --all-features --locked
run "test"         cargo test --workspace --locked
run "clippy"       cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" run "doc" cargo doc --workspace --no-deps --locked
# The game library is meant to have no platform dependencies at all.
run "no-frontend"  cargo build -p robin --no-default-features --locked
# What the dependency tree is allowed to contain. Skipped rather than failed
# when the tool is absent, since it is the one check here that needs an
# install: `cargo install cargo-deny --locked`.
if command -v cargo-deny > /dev/null; then
  run "cargo-deny"  cargo deny check
else
  echo "!!! cargo-deny not installed; the dependency policy was NOT checked."
fi

# The attributions shipped with a binary. Same story: an install away, and
# worth saying loudly when it did not run.
if command -v cargo-about > /dev/null; then
  echo "=== third-party attributions"
  cargo about generate --all-features about.hbs -o "${TMPDIR:-/tmp}/THIRD-PARTY.md" 2> /dev/null
  if diff -q THIRD-PARTY.md "${TMPDIR:-/tmp}/THIRD-PARTY.md" > /dev/null; then
    echo "THIRD-PARTY.md is current"
  else
    failed+=("THIRD-PARTY.md is stale: cargo about generate --all-features about.hbs -o THIRD-PARTY.md")
  fi
else
  echo "!!! cargo-about not installed; THIRD-PARTY.md was NOT checked."
fi

# The differential suites: the rewrite against the original, byte for byte
# (tools/robin-verify, #24). They need the player's own tape and the 128K ROM,
# which CI has not got, so this script is the only place they run. Without
# them the gate FAILS rather than passing quietly: a check that didn't run
# must not look like one that passed (#6, Decision 9).
echo "=== differential suites (robin-verify)"
if cargo run --release -q -p robin-verify -- assets; then
  :
else
  failed+=("differential suites (robin-verify): see above; they need the tape and 128.rom in assets/")
fi

# The machines' timing against test programs measured on real machines
# (crates/zx-runtime/tests/hardware.rs, #12). They run in `cargo test` above,
# but need the real ROMs and the programs in assets/, which CI has not got;
# without them they pass by saying they were skipped, so say it here too.
missing=""
for f in 128.rom 48.rom minfo.tap fusetest.tap timingtest.tap butler-128k.szx; do
  [ -f "assets/$f" ] || missing="$missing $f"
done
if [ -n "$missing" ]; then
  echo "!!! hardware timing tests SKIPPED for want of:$missing (see assets/README.md)"
fi

if [ ${#failed[@]} -ne 0 ]; then
  printf 'FAILED: %s\n' "${failed[@]}"
  exit 1
fi
echo "all checks passed"
