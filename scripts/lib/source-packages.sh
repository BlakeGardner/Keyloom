# Shared helpers for the scripts that build Keyloom's source packages: a
# Debian source package (.dsc and tarballs) and an RPM spec file that
# builds from the same orig tarball. Source this file; it defines
# functions only.

# Writes a one-entry debian/changelog.
# Usage: write_debian_changelog OUTPUT SOURCE_NAME VERSION DATE_RFC2822 LINE...
# The maintainer is taken from the debian/control next to OUTPUT.
write_debian_changelog() {
    local output=$1 name=$2 version=$3 date=$4
    shift 4
    local maintainer
    maintainer=$(sed -n 's/^Maintainer: //p' "$(dirname "$output")/control")
    {
        printf '%s (%s) unstable; urgency=medium\n\n' "$name" "$version"
        local line
        for line in "$@"; do
            printf '  * %s\n' "$line"
        done
        printf '\n -- %s  %s\n' "$maintainer" "$date"
    } > "$output"
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

# Writes an RPM spec file from a template, filling in @VERSION@, @DATE@
# (the changelog entry's date, from DATE_RFC2822), and @NOTES@ (the
# entry's one line).
# Usage: write_rpm_spec TEMPLATE OUTPUT VERSION DATE_RFC2822 NOTES
write_rpm_spec() {
    local template=$1 output=$2 version=$3 date=$4 notes=$5
    local stamp
    stamp=$(LC_ALL=C date -u -d "$date" '+%a %b %d %Y')
    sed -e "s|@VERSION@|$(sed_replacement "$version")|g" \
        -e "s|@DATE@|$(sed_replacement "$stamp")|g" \
        -e "s|@NOTES@|$(sed_replacement "$notes")|g" "$template" > "$output"
    if grep -q '@[A-Z_]*@' "$output"; then
        echo "$template has a placeholder this script does not fill: $(grep -o '@[A-Z_]*@' "$output" | sort -u | tr '\n' ' ')" >&2
        return 1
    fi
}

# Escapes TEXT for the replacement part of a sed s|||g command.
sed_replacement() {
    printf '%s' "$1" | sed 's/[\\&|]/\\&/g'
}
