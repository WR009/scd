#!/usr/bin/env bash
# Build scd and install it in the layout described by docs/ARCHITECTURE.md:
#
#   <dest>/bin/scd        the program        (put this directory on PATH)
#   <dest>/shell/*        shell functions that perform the real `cd`
#   <dest>/config.json    your locations and settings (kept across upgrades)
#
# scd never modifies environment variables itself; --add-to-rc makes this script append
# the PATH export and the `source` line to your shell startup file, or do it by hand.

set -euo pipefail

# Colour, only when it will actually be seen - same rules as the program itself.
# These must be defined BEFORE argument parsing, or the error paths below would call a
# function that does not exist yet and `set -e` would abort with exit 127 and no message.
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-}" != "dumb" ]; then
    C_CYAN=$'\033[1;36m'; C_GREEN=$'\033[1;32m'; C_YELLOW=$'\033[33m'
    C_RED=$'\033[1;31m'; C_DIM=$'\033[2m'; C_OFF=$'\033[0m'
else
    C_CYAN=''; C_GREEN=''; C_YELLOW=''; C_RED=''; C_DIM=''; C_OFF=''
fi
say()  { printf '%s==> %s%s\n' "$C_CYAN" "$1" "$C_OFF"; }
good() { printf '%s%s%s\n' "$C_GREEN" "$1" "$C_OFF"; }
note() { printf '%s%s%s\n' "$C_DIM" "$1" "$C_OFF"; }
# Errors go to stderr, so they get their own tty check: CI logs stay escape-free.
bad() {
    if [ -t 2 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-}" != "dumb" ]; then
        printf '%s%s%s\n' "$C_RED" "$1" "$C_OFF" >&2
    else
        printf '%s\n' "$1" >&2
    fi
}

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
  --add-to-rc      add <dest>/bin to PATH and load the scd function from your rc file
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

# A release archive carries a prebuilt bin/scd and no Cargo.toml: nothing to build.
if [ ! -f "$REPO/Cargo.toml" ] && [ -f "$REPO/bin/scd" ]; then
    say "release archive detected: using $REPO/bin/scd (no Rust toolchain needed)"
    BUILT="$REPO/bin/scd"
else
    if [ "$SKIP_BUILD" -eq 0 ]; then
        if ! command -v cargo >/dev/null 2>&1; then
            if [ -f "$REPO/bin/scd" ]; then
                say 'cargo is not on PATH - falling back to the prebuilt binary'
                BUILT="$REPO/bin/scd"
            else
                bad 'cargo is not on PATH. Install Rust (https://rustup.rs) or use --skip-build.'
                exit 1
            fi
        else
            say "cargo build --$CONFIGURATION (no dependencies, works offline)"
            (cd "$REPO" && cargo build "--$CONFIGURATION")
            BUILT="$REPO/target/$CONFIGURATION/scd"
        fi
    else
        BUILT="$REPO/target/$CONFIGURATION/scd"
        [ -f "$BUILT" ] || BUILT="$REPO/bin/scd"
    fi
fi

if [ ! -f "$BUILT" ]; then
    bad "no binary found (looked for $REPO/bin/scd and $REPO/target/$CONFIGURATION/scd)"
    exit 1
fi

say "installing into $DEST"
mkdir -p "$DEST/bin" "$DEST/shell"
cp -f "$BUILT" "$DEST/bin/scd"
chmod +x "$DEST/bin/scd"
cp -f "$REPO"/shell/scd.bash "$REPO"/shell/scd.zsh "$REPO"/shell/scd.fish "$DEST/shell/"
cp -f "$REPO"/shell/scd.ps1 "$REPO"/shell/scd.cmd "$DEST/shell/" 2>/dev/null || true
for extra in install.sh install.ps1 install.cmd README.md CHANGELOG.md LICENSE config.example.json; do
    if [ -f "$REPO/$extra" ] && [ "$(cd "$REPO" && pwd)/$extra" != "$(cd "$DEST" && pwd)/$extra" ]; then
        cp -f "$REPO/$extra" "$DEST/$extra"
    fi
done
chmod +x "$DEST/install.sh" 2>/dev/null || true

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
            case ":$PATH:" in
                *":$DEST/bin:"*) ;;
                *) printf 'export PATH="%s/bin:$PATH"\n' "$DEST" ;;
            esac
            printf 'source "%s"\n' "$WRAPPER"
        } >>"$RC_FILE"
        say "added the scd function to $RC_FILE"
    fi
fi

printf '\n'
good "scd is installed in $DEST"
printf '\n'
if [ "$ADD_TO_RC" -eq 1 ]; then
    printf '%s1) PATH and the scd function were added to %s:%s\n' "$C_YELLOW" "$RC_FILE" "$C_OFF"
    note '     open a NEW terminal, then type:  scd'
else
    printf '%s1) put the program on PATH:%s\n' "$C_YELLOW" "$C_OFF"
    note "     export PATH=\"$DEST/bin:\$PATH\""
    note "   (add that line to $RC_FILE to keep it)"
    printf '\n'
    printf '%s2) load the shell function so "scd" really changes directory:%s\n' "$C_YELLOW" "$C_OFF"
    note "     source \"$WRAPPER\""
    note '   or re-run with --add-to-rc and both steps are done for you'
fi
printf '\n'
printf '%stry it:%s\n' "$C_YELLOW" "$C_OFF"
note '     scd            # the [c]/[a]/[1..n] menu'
note '     scd add .      # save the current directory'
note '     scd <name>     # jump straight there'
note '     scd --help     # the full user guide'
