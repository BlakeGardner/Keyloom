# Shared helpers for the scripts that build Keyloom's Debian source
# packages. Source this file; it defines functions only.

# Writes a one-entry debian/changelog.
# Usage: write_debian_changelog PATH SOURCE_NAME VERSION DATE_RFC2822 LINE...
# The maintainer is taken from the debian/control next to PATH.
write_debian_changelog() {
    local path=$1 name=$2 version=$3 date=$4
    shift 4
    local maintainer
    maintainer=$(sed -n 's/^Maintainer: //p' "$(dirname "$path")/control")
    {
        printf '%s (%s) unstable; urgency=medium\n\n' "$name" "$version"
        local line
        for line in "$@"; do
            printf '  * %s\n' "$line"
        done
        printf '\n -- %s  %s\n' "$maintainer" "$date"
    } > "$path"
}

# Prints the files a `dpkg-source -b` run produced for SOURCE_NAME VERSION
# in DIR: the .dsc and the tarballs it references.
debian_source_files() {
    local dir=$1 name=$2 version=$3
    local dsc="$dir/${name}_$version.dsc"
    local tarballs
    tarballs=$(sed -n '/^Files:/,/^[^ ]/{s/^ [0-9a-f]* [0-9]* //p}' "$dsc")
    printf '%s\n' "$dsc"
    printf '%s\n' "$tarballs" | sed "s|^|$dir/|"
}
