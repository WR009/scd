#!/usr/bin/env bash
# Build scd and install it in the layout described by docs/ARCHITECTURE.md:
#
#   <dest>/bin/scd        the program        (put this directory on PATH)
#   <dest>/shell/*        shell functions that perform the real `cd`
#   <dest>/config.json    your locations and settings (kept across upgrades)
#
# scd never modifies environment variables itself; use --add-to-rc if you want this
# script to append the `source` line to your shell startup file, or do it by hand.

set -euo pipefail

DEST="${HOME}/scd"
CONFIGURATION="release"
SKIP_BUILD=0
ADD_TO_RC=0

usage() {
    cat <<'USAGE'
usage: install.sh [options]

options:
  --dest <dir>     install directory (default: $HOME/scd)
  --debug          build the debug profile instead of release
  --skip-build     reuse an existing target/<profile>/scd
  --add-to-rc      append the `source` line to ~/.bashrc, ~/.zshrc or config.fish
  -h, --help       show this help
USAGE
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dest) DEST="${2:?--dest needs a directory}"; shift 2 ;;
        --dest=*) DEST="${1#*=}"; shift ;;
        --debug) CONFIGURATION="debug"; shift ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        --add-to-rc) ADD_TO_RC=1; shift ;;
        -h | --help) usage; exit 0 ;;
        *) bad "install.sh: unknown argument $1"; printf '\n' >&2; usage >&2; exit 64 ;;
    esac
done

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colour, only when it will actually be seen: same rules as the program itself.
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-}" != "dumb" ]; then
    C_CYAN=$'\033[1;36m'; C_GREEN=$'\033[1;32m'; C_YELLOW=$'\033[33m'
    C_RED=$'\033[1;31m'; C_DIM=$'\033[2m'; C_OFF=$'\033[0m'
else
    C_CYAN=''; C_GREEN=''; C_YELLOW=''; C_RED=''; C_DIM=''; C_OFF=''
fi
say()  { printf '%s==> %s%s\n' "$C_CYAN" "$1" "$C_OFF"; }
good() { printf '%s%s%s\n' "$C_GREEN" "$1" "$C_OFF"; }
note() { printf '%s%s%s\n' "$C_DIM" "$1" "$C_OFF"; }
bad()  { printf '%s%s%s\n' "$C_RED" "$1" "$C_OFF" >&2; }

if [ "$SKIP_BUILD" -eq 0 ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        bad 'cargo is not on PATH. Install Rust (https://rustup.rs) or use --skip-build.'
        exit 1
    fi
    say "cargo build --$CONFIGURATION (no dependencies, works offline)"
    (cd "$REPO" && cargo build "--$CONFIGURATION")
fi

BUILT="$REPO/target/$CONFIGURATION/scd"
if [ ! -f "$BUILT" ]; then
    bad "no build found at $BUILT"
    exit 1
fi

say "installing into $DEST"
mkdir -p "$DEST/bin" "$DEST/shell"
cp -f "$BUILT" "$DEST/bin/scd"
chmod +x "$DEST/bin/scd"
cp -f "$REPO"/shell/scd.bash "$REPO"/shell/scd.zsh "$REPO"/shell/scd.fish "$DEST/shell/"
cp -f "$REPO"/shell/scd.ps1 "$REPO"/shell/scd.cmd "$DEST/shell/" 2>/dev/null || true

if [ ! -f "$DEST/config.json" ]; then
    cat >"$DEST/config.json" <<'JSON'
{
  "version": 1,
  "max_display": 9,
  "locations": []
}
JSON
    say "created $DEST/config.json"
else
    say "keeping the existing $DEST/config.json"
fi

"$DEST/bin/scd" --version

# Which shell function to load, based on the shell that is actually in use.
case "${SHELL:-/bin/bash}" in
    */zsh) RC_FILE="${HOME}/.zshrc"; WRAPPER="$DEST/shell/scd.zsh" ;;
    */fish) RC_FILE="${HOME}/.config/fish/config.fish"; WRAPPER="$DEST/shell/scd.fish" ;;
    *) RC_FILE="${HOME}/.bashrc"; WRAPPER="$DEST/shell/scd.bash" ;;
esac

if [ "$ADD_TO_RC" -eq 1 ]; then
    mkdir -p "$(dirname "$RC_FILE")"
    if [ -f "$RC_FILE" ] && grep -Fq "$WRAPPER" "$RC_FILE"; then
        say "$RC_FILE already loads scd"
    else
        {
            printf '\n# scd - super cd (added by install.sh)\n'
            printf 'source "%s"\n' "$WRAPPER"
        } >>"$RC_FILE"
        say "added the scd function to $RC_FILE"
    fi
fi

printf '\n'
good "scd is installed in $DEST"
printf '\n'
printf '%s1) put the program on PATH:%s\n' "$C_YELLOW" "$C_OFF"
note "     export PATH=\"$DEST/bin:\$PATH\""
note "   (add that line to $RC_FILE to keep it)"
printf '\n'
if [ "$ADD_TO_RC" -eq 0 ]; then
    printf '%s2) load the shell function so "scd" really changes directory:%s\n' "$C_YELLOW" "$C_OFF"
    note "     source \"$WRAPPER\""
    note "   or re-run with --add-to-rc to append it to $RC_FILE"
    printf '\n'
fi
printf '%s3) try it:%s\n' "$C_YELLOW" "$C_OFF"
note '     scd            # the [c]/[a]/[1..n] menu'
note '     scd add .      # save the current directory'
note '     scd <name>     # jump straight there'
note '     scd --help     # the full user guide'
