#!/usr/bin/env bash
#
# qa-reset.sh — put a test machine back to "never set up" for Keyloom QA.
#
# Keyloom's first-run setup changes a few things on a system. This script
# undoes all of them, then removes Keyloom's own saved state, the xremap
# package, and Keyloom's own download of xremap, so the wizard can be
# exercised again from scratch:
#
#   1. The xremap.service systemd *user* unit Keyloom installs
#      (~/.config/systemd/user/xremap.service) — stopped, disabled, and
#      deleted, together with any backups setup made of a previous unit.
#   2. Membership in the `input` group (read access to keyboards) — the
#      current user is removed from the group.
#   3. Access to /dev/uinput (the virtual keyboard) — the udev rule and the
#      modules-load.d entry are deleted, udev is reloaded, and the uinput
#      module is unloaded so the device disappears.
#   4. Keyloom's generated xremap configuration (~/.config/xremap/keyloom.yml
#      and its backups) and Keyloom's settings store
#      (~/.config/cosmic/io.github.blakegardner.Keyloom: profiles, layouts,
#      and the "setup complete" record).
#   5. xremap itself, when it was installed from a Debian/Ubuntu package
#      (xremap, xremap-gnome, xremap-cosmic, ...): purged with apt-get, which
#      also removes the udev rule the package ships. On systems without
#      apt, or with an xremap that no package owns, this step only reports
#      what it found.
#   6. Keyloom's own download of xremap (~/.local/bin/xremap), when the
#      file there is byte for byte a release Keyloom downloads (checked
#      against the digests in src/install.rs). A binary placed there by
#      hand is reported and left alone.
#
# What it does NOT touch:
#   - Hand-written xremap configs. Only keyloom.yml* is removed.
#   - A udev rule under /usr/lib/udev/rules.d that some other package owns.
#     The script warns when one is left, because the "virtual keyboard"
#     step will still pass with it in place.
#
# Intended for a disposable VM. It deletes files, changes group membership,
# and uninstalls a package; it summarizes all of that and asks for a "yes"
# before starting. Do not run it on a machine whose xremap setup you care
# about.
#
# Usage:
#   scripts/qa-reset.sh            # summarize, ask for confirmation, do it
#   scripts/qa-reset.sh --yes      # skip the confirmation (scripted runs)
#   scripts/qa-reset.sh --dry-run  # print what would be done, change nothing
#
# Run it as the normal desktop user, not as root: the user unit belongs to
# that user's systemd instance. Steps that need root use sudo and prompt
# for a password once. Log out and back in (or reboot) afterwards: group
# membership is read at login, and a reboot also clears the uinput access
# the session may still hold.

set -euo pipefail

DRY_RUN=0
ASSUME_YES=0
case "${1:-}" in
    --dry-run|-n) DRY_RUN=1 ;;
    --yes|-y) ASSUME_YES=1 ;;
    "") ;;
    -h|--help)
        sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "unknown option: $1 (try --help)" >&2
        exit 2
        ;;
esac

if [ "$(id -u)" -eq 0 ]; then
    echo "run this as the desktop user, not as root; it uses sudo where needed" >&2
    exit 1
fi

# --- helpers -----------------------------------------------------------------

# Print a command, then run it unless this is a dry run. Failures inside a
# step are reported but do not stop the script: a missing file or an
# already-stopped unit is exactly what a partly reset machine looks like.
run() {
    echo "  \$ $*"
    if [ "$DRY_RUN" -eq 0 ]; then
        "$@" || echo "  (failed, continuing)"
    fi
}

# Same, but as root through sudo.
run_root() {
    run sudo "$@"
}

step() {
    echo
    echo "== $*"
}

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
UNIT_DIR="$CONFIG_HOME/systemd/user"
UNIT="$UNIT_DIR/xremap.service"
KEYLOOM_YML="$CONFIG_HOME/xremap/keyloom.yml"
KEYLOOM_STORE="$CONFIG_HOME/cosmic/io.github.blakegardner.Keyloom"
RULES_FILE=/etc/udev/rules.d/00-xremap-input.rules
MODULES_FILE=/etc/modules-load.d/uinput.conf
INPUT_GROUP=input
DOWNLOADED_XREMAP="$HOME/.local/bin/xremap"

# --- confirmation ------------------------------------------------------------

# Installed xremap packages, one per line (empty when none, or without dpkg).
xremap_packages() {
    command -v dpkg-query >/dev/null 2>&1 || return 0
    dpkg-query -W -f='${Package} ${Status}\n' 'xremap*' 2>/dev/null \
        | awk '$NF == "installed" { print $1 }'
}

# Whether a file is byte for byte one of the xremap releases Keyloom
# downloads, judged by the digests recorded in src/install.rs.
is_keyloom_download() {
    command -v sha256sum >/dev/null 2>&1 || return 1
    digest=$(sha256sum -- "$1" | cut -d' ' -f1)
    grep -q "\"$digest\"" "$(dirname "$0")/../src/install.rs"
}

echo "This resets Keyloom's first-run setup on this machine for user $USER:"
echo "  - stop, disable, and delete $UNIT (and its backups)"
echo "  - remove $USER from the $INPUT_GROUP group"
echo "  - delete $RULES_FILE and $MODULES_FILE, reload udev, unload uinput"
echo "  - delete $KEYLOOM_YML* and $KEYLOOM_STORE"
packages="$(xremap_packages)"
if [ -n "$packages" ]; then
    echo "  - purge the xremap package(s): $(echo "$packages" | tr '\n' ' ')"
elif command -v xremap >/dev/null 2>&1; then
    echo "  - xremap at $(command -v xremap) is not from a Debian package and stays installed"
else
    echo "  - xremap is not installed"
fi
if [ -e "$DOWNLOADED_XREMAP" ]; then
    if is_keyloom_download "$DOWNLOADED_XREMAP"; then
        echo "  - delete Keyloom's downloaded xremap, $DOWNLOADED_XREMAP"
    else
        echo "  - $DOWNLOADED_XREMAP was not downloaded by Keyloom and stays"
    fi
fi
echo "Steps marked with sudo ask for your password."

if [ "$DRY_RUN" -eq 1 ]; then
    echo
    echo "dry run: nothing below is executed"
elif [ "$ASSUME_YES" -eq 0 ]; then
    echo
    read -r -p "Type yes to continue, anything else to stop: " answer
    if [ "$answer" != "yes" ]; then
        echo "Nothing was changed."
        exit 0
    fi
fi

# Ask for the sudo password up front so the privileged steps do not stall
# halfway through.
if [ "$DRY_RUN" -eq 0 ]; then
    sudo -v
fi

# --- 0. Keyloom itself -------------------------------------------------------

step "Closing Keyloom if it is running"
# A running Keyloom would rewrite keyloom.yml and its settings as they are
# removed. pkill returns 1 when nothing matched, which is fine.
run pkill -x keyloom

# --- 1. The systemd user unit ------------------------------------------------

step "Removing the xremap.service user unit"
# disable --now stops the unit and removes the graphical-session.target.wants
# link that setup's `enable` created. It must run before the file goes away,
# or the link would be left dangling.
run systemctl --user disable --now xremap.service
# The unit file plus any backups setup made when it replaced an existing
# unit (xremap.service.bak, .bak.1, ...), and a leftover temp file.
for f in "$UNIT" "$UNIT".bak "$UNIT".bak.* "$UNIT".tmp; do
    [ -e "$f" ] && run rm -f -- "$f"
done
run systemctl --user daemon-reload
run systemctl --user reset-failed xremap.service
# Anything else still running xremap (a hand-started one, say) would keep
# holding the keyboards.
run pkill -x xremap

# --- 2. The xremap package ---------------------------------------------------

step "Uninstalling xremap"
if [ -n "$packages" ]; then
    # Purge rather than remove: the package's udev rule and any conffiles go
    # with it, which is what leaves the "virtual keyboard" step testable.
    # shellcheck disable=SC2086  # package names are separate arguments
    run_root apt-get purge -y $packages
elif command -v xremap >/dev/null 2>&1; then
    echo "  $(command -v xremap) was not installed by apt; remove it by hand if the"
    echo "  'xremap missing' path is being tested"
else
    echo "  not installed"
fi

# --- 2b. Keyloom's own download of xremap ------------------------------------

step "Removing Keyloom's downloaded xremap"
if [ -e "$DOWNLOADED_XREMAP" ]; then
    if is_keyloom_download "$DOWNLOADED_XREMAP"; then
        run rm -f -- "$DOWNLOADED_XREMAP"
    else
        echo "  $DOWNLOADED_XREMAP is not a release Keyloom downloads; remove it by hand"
        echo "  if the 'xremap missing' path is being tested"
    fi
else
    echo "  none"
fi

# --- 3. The input group ------------------------------------------------------

step "Leaving the $INPUT_GROUP group"
if id -nG | tr ' ' '\n' | grep -qx "$INPUT_GROUP" \
    || getent group "$INPUT_GROUP" | cut -d: -f4 | tr ',' '\n' | grep -qx "$USER"; then
    run_root gpasswd -d "$USER" "$INPUT_GROUP"
else
    echo "  $USER is not in $INPUT_GROUP"
fi

# --- 4. uinput access --------------------------------------------------------

step "Removing uinput access"
[ -e "$RULES_FILE" ] && run_root rm -f -- "$RULES_FILE"
[ -e "$MODULES_FILE" ] && run_root rm -f -- "$MODULES_FILE"
run_root udevadm control --reload-rules
# Re-apply the remaining rules to the device so the uaccess ACL the removed
# rule granted is dropped, then unload the module so /dev/uinput goes away
# entirely (the "not enabled" state). Unloading fails harmlessly if the
# module is built into the kernel or something still has the device open.
run_root udevadm trigger --subsystem-match=misc --sysname-match=uinput
run_root udevadm settle
run_root modprobe -r uinput

# A rule some other package owns cannot be reset from here; say so.
for dir in /usr/lib/udev/rules.d /lib/udev/rules.d /usr/local/lib/udev/rules.d; do
    if [ -e "$dir/00-xremap-input.rules" ]; then
        echo "  note: $dir/00-xremap-input.rules belongs to a package and was left alone;"
        echo "        the virtual keyboard step will pass as long as it is there."
    fi
done

# --- 5. Keyloom's files ------------------------------------------------------

step "Removing Keyloom's generated configuration and settings"
for f in "$KEYLOOM_YML" "$KEYLOOM_YML".bak "$KEYLOOM_YML".bak.* "$KEYLOOM_YML".tmp; do
    [ -e "$f" ] && run rm -f -- "$f"
done
[ -e "$KEYLOOM_STORE" ] && run rm -rf -- "$KEYLOOM_STORE"

# --- done --------------------------------------------------------------------

echo
if [ "$DRY_RUN" -eq 1 ]; then
    echo "Dry run finished; nothing was changed."
else
    echo "Reset finished. Log out and back in (or reboot) before testing:"
    echo "  - leaving the $INPUT_GROUP group takes effect at the next login"
    echo "  - a reboot also clears any uinput access this session still holds"
    echo "Keyloom will open first-run setup on its next launch."
fi
