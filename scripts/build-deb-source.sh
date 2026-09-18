#!/usr/bin/env bash
# Builds Keyloom's Debian source package for the Open Build Service: a .dsc,
# an orig tarball with the sources and the vendored crates, and the
# debian tarball. It never builds the binary itself; the build service does
# that, offline, with the keyloom-rust-toolchain package. See
# packaging/README.md.
#
# Usage: scripts/build-deb-source.sh OUTPUT_DIR [--tag TAG] [--ref REF]
#   --tag TAG   Fail unless Cargo.toml's version matches TAG (v1.2.3 or 1.2.3).
#   --ref REF   Git ref to package (default: HEAD). Only committed files are
#               packaged; packaging/debian is taken from the working tree.
#
# Needs cargo, cargo-vendor-filterer, dpkg-source, git, tar, and xz, plus
# network access to fetch the crates.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck source=scripts/lib/debian-source.sh
. "$repo/scripts/lib/debian-source.sh"

out=${1:?usage: $0 OUTPUT_DIR [--tag TAG] [--ref REF]}
shift
tag=
ref=HEAD
while [ $# -gt 0 ]; do
    case $1 in
        --tag) tag=${2:?--tag needs a value}; shift 2 ;;
        --ref) ref=${2:?--ref needs a value}; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
mkdir -p "$out"
out=$(cd "$out" && pwd)

if ! cargo vendor-filterer --version >/dev/null 2>&1; then
    echo "cargo-vendor-filterer is missing: cargo install --locked cargo-vendor-filterer" >&2
    exit 1
fi

# The [package] table is the first table in Cargo.toml.
version=$(awk -F'"' '/^\[package\]/ { p = 1; next } /^\[/ { p = 0 }
    p && /^version *=/ { print $2; exit }' "$repo/Cargo.toml")
if [ -n "$tag" ] && [ "${tag#v}" != "$version" ]; then
    echo "Tag $tag does not match Cargo.toml version $version" >&2
    exit 1
fi
# Debian sorts 1.0.0~rc.1 before 1.0.0, as a pre-release should.
debian_version=${version//-/\~}
name=keyloom
srcdir="$name-$debian_version"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "Archiving $ref as $srcdir"
git -C "$repo" archive --format=tar --prefix="$srcdir/" "$ref" | tar -C "$work" -xf -
commit_date=$(git -C "$repo" log -1 --format=%cD "$ref")

echo "Vendoring crates for x86_64 and aarch64 Linux"
(
    cd "$work/$srcdir"
    cargo vendor-filterer --platform=x86_64-unknown-linux-gnu \
        --platform=aarch64-unknown-linux-gnu --keep-dep-kinds=no-dev \
        vendor > "$work/cargo-config.toml"
    mkdir .cargo
    sed 's|^directory = .*|directory = "vendor"|' "$work/cargo-config.toml" \
        > .cargo/config.toml
)

echo "Creating the orig tarball"
tar -C "$work" -cf - "$srcdir" | xz -T0 > "$work/${name}_$debian_version.orig.tar.xz"

cp -r "$repo/packaging/debian" "$work/$srcdir/debian"
write_debian_changelog "$work/$srcdir/debian/changelog" "$name" "$debian_version-1" \
    "$commit_date" \
    "Keyloom $version. Release notes: https://github.com/BlakeGardner/Keyloom/releases/tag/${tag:-$version}"

(cd "$work" && dpkg-source -b "$srcdir")

mapfile -t files < <(debian_source_files "$work" "$name" "$debian_version-1")
for file in "${files[@]}"; do
    mv "$file" "$out/"
    echo "  $out/$(basename "$file")"
done
