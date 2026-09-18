#!/usr/bin/env bash
# Builds the keyloom-rust-toolchain Debian source package: upstream's rustc,
# rust-std, and cargo release tarballs for x86_64 and aarch64, verified
# against their published checksums, wrapped as an orig tarball with the
# packaging from packaging/rust-toolchain. The version comes from
# packaging/rust-toolchain/version. See packaging/README.md.
#
# Usage: scripts/build-rust-toolchain-source.sh OUTPUT_DIR
#
# Downloads are kept in $RUST_DIST_CACHE (default:
# ~/.cache/keyloom-rust-dist) and reused on later runs.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck source=scripts/lib/debian-source.sh
. "$repo/scripts/lib/debian-source.sh"

out=${1:?usage: $0 OUTPUT_DIR}
mkdir -p "$out"
out=$(cd "$out" && pwd)
version=$(tr -d '[:space:]' < "$repo/packaging/rust-toolchain/version")
name=keyloom-rust-toolchain
srcdir="$name-$version"
dist=https://static.rust-lang.org/dist
triples=(x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu)
components=(rustc rust-std cargo)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/$srcdir"

cache=${RUST_DIST_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/keyloom-rust-dist}
mkdir -p "$cache"
for triple in "${triples[@]}"; do
    for component in "${components[@]}"; do
        file="$component-$version-$triple.tar.xz"
        if [ ! -f "$cache/$file" ]; then
            echo "Downloading $file"
            curl -sSfL -o "$cache/$file.part" "$dist/$file"
            mv "$cache/$file.part" "$cache/$file"
        fi
        curl -sSfL -o "$cache/$file.sha256" "$dist/$file.sha256"
        (cd "$cache" && sha256sum --check --quiet "$file.sha256")
        cp "$cache/$file" "$work/$srcdir/"
    done
done

cat > "$work/$srcdir/README" <<README
Rust $version release tarballs from $dist, verified against the
SHA-256 sums published there. debian/rules installs the set for the build
architecture under /usr/lib/keyloom-rust-toolchain.
README

echo "Creating the orig tarball"
# The tarballs are already xz-compressed; a light gzip keeps them verbatim.
tar -C "$work" -cf - "$srcdir" | gzip -1 > "$work/${name}_$version.orig.tar.gz"

cp -r "$repo/packaging/rust-toolchain/debian" "$work/$srcdir/debian"
write_debian_changelog "$work/$srcdir/debian/changelog" "$name" "$version-1" \
    "$(date -Ru)" "Rust $version from the upstream release tarballs."

(cd "$work" && dpkg-source -b "$srcdir")

mapfile -t files < <(debian_source_files "$work" "$name" "$version-1")
for file in "${files[@]}"; do
    mv "$file" "$out/"
    echo "  $out/$(basename "$file")"
done
