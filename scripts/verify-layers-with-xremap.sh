#!/bin/sh
# Run Keyloom's generated layer configurations through xremap's own
# event-handler tests, so the semantics (not just the syntax) of what the
# generator emits are checked against the pinned xremap release.
#
# Builds the xremap source at the pinned tag (kept in sync with
# XREMAP_VERSION in .github/workflows/ci.yml and install::RELEASE in
# src/install.rs, the release setup downloads) under target/, adds
# scripts/xremap-harness/tests_keyloom.rs to its test suite, and runs it
# on documents dumped by Keyloom's generator.
#
# Safe on a machine that uses xremap: nothing here runs an xremap binary,
# opens an input device, or reads the installed xremap or its
# configuration. The checkout and its build live under
# XREMAP_HARNESS_DIR (default target/xremap-harness); the first run needs
# network access for the clone and a few minutes to build.
set -eu

version=${XREMAP_VERSION:-0.15.13}
root=$(cd "$(dirname "$0")/.." && pwd)
work=${XREMAP_HARNESS_DIR:-"$root/target/xremap-harness"}
src="$work/xremap-$version"

if [ ! -d "$src" ]; then
    mkdir -p "$work"
    git clone --quiet --depth 1 --branch "v$version" \
        https://github.com/xremap/xremap.git "$src"
fi

mkdir -p "$src/src/tests/keyloom"
KEYLOOM_HARNESS_DIR="$src/src/tests/keyloom" cargo test --locked \
    --manifest-path "$root/Cargo.toml" \
    xremap::tests::dump_documents_for_the_xremap_harness -- --ignored --exact
cp "$root/scripts/xremap-harness/tests_keyloom.rs" "$src/src/tests/"
grep -q '^mod tests_keyloom;' "$src/src/tests/mod.rs" \
    || printf 'mod tests_keyloom;\n' >> "$src/src/tests/mod.rs"

cd "$src"
cargo test --locked --lib tests_keyloom
