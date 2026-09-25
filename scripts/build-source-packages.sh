#!/usr/bin/env bash
# Builds Keyloom's source packages for the Open Build Service: a Debian
# source package (a .dsc, an orig tarball with the sources and the vendored
# crates, and the debian tarball) and an RPM spec file that builds from the
# same orig tarball. It never builds the binaries itself; the build service
# does that, offline, with the keyloom-rust-toolchain package. See
# packaging/README.md.
#
# Usage: scripts/build-source-packages.sh OUTPUT_DIR [--tag TAG] [--ref REF]
#   --ref REF   Git ref to package (default: HEAD). Only committed files are
#               packaged, and the version is the ref's Cargo.toml version;
#               packaging/debian and packaging/rpm are taken from the working
#               tree, so an older release can be packaged with current recipes.
#   --tag TAG   Fail unless TAG is that version with a leading v (v1.2.3).
#
# Needs cargo, cargo-vendor-filterer, dpkg-source, git, tar, and xz, plus
# network access to fetch the crates.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck source=scripts/lib/source-packages.sh
. "$repo/scripts/lib/source-packages.sh"

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
version=$(git -C "$repo" show "$ref:Cargo.toml" | awk -F'"' '/^\[package\]/ { p = 1; next } /^\[/ { p = 0 }
    p && /^version *=/ { print $2; exit }')
if [ -z "$version" ]; then
    echo "Could not read the version from Cargo.toml at $ref" >&2
    exit 1
fi
if [ -n "$tag" ] && [ "$tag" != "v$version" ]; then
    echo "Tag $tag does not match the Cargo.toml version $version at $ref (expected v$version)" >&2
    exit 1
fi
# Debian and RPM both sort 1.0.0~rc.1 before 1.0.0, as a pre-release should.
package_version=${version//-/\~}
name=keyloom
srcdir="$name-$package_version"
notes="Keyloom $version. Release notes: https://github.com/BlakeGardner/Keyloom/releases/tag/v$version"

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
    # The repository's own .cargo/config.toml (the interface tests'
    # renderer) stays; the vendored sources are appended to it.
    mkdir -p .cargo
    if [ -s .cargo/config.toml ]; then echo >> .cargo/config.toml; fi
    sed 's|^directory = .*|directory = "vendor"|' "$work/cargo-config.toml" \
        >> .cargo/config.toml
)

echo "Creating the orig tarball"
tar -C "$work" -cf - "$srcdir" | xz -T0 > "$work/${name}_$package_version.orig.tar.xz"

cp -r "$repo/packaging/debian" "$work/$srcdir/debian"
write_debian_changelog "$work/$srcdir/debian/changelog" "$name" "$package_version-1" \
    "$commit_date" "$notes"

(cd "$work" && dpkg-source -b "$srcdir")

mapfile -t files < <(debian_source_files "$work" "$name" "$package_version-1")
for file in "${files[@]}"; do
    mv "$file" "$out/"
    echo "  $out/$(basename "$file")"
done

echo "Writing the RPM spec"
write_rpm_spec "$repo/packaging/rpm/$name.spec.in" "$out/$name.spec" "$package_version" \
    "$commit_date" "$notes"
echo "  $out/$name.spec"
