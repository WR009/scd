#!/usr/bin/env bash
# Package one release asset. This is what CI runs for every platform, and what you run on
# Linux/macOS by hand. tools/dist.ps1 is the Windows equivalent; both produce the same layout.
#
#   tools/dist.sh --target x86_64-unknown-linux-gnu --label Linux_x64 --ext tar.gz
#   tools/dist.sh --target x86_64-unknown-linux-musl --label Linux_x64_static --ext tar.gz --skip-build
#
# Output under ./releases :
#   <label>/                       unpacked payload (the documented install layout)
#   scd-<version>-<target>.<ext>   the file you upload to a GitHub Release
#   <asset>.sha256                 checksum in `sha256sum -c` format
#
# config.json is deliberately NOT packaged: it is user state.

set -euo pipefail

TARGET=""
LABEL=""
EXT="tar.gz"
VERSION=""
SKIP_BUILD=0

usage() {
    cat <<'USAGE'
usage: tools/dist.sh [options]

options:
  --target <triple>   rust target triple (default: the host triple)
  --label <name>      folder name for the unpacked payload (default: derived from --target)
  --ext <zip|tar.gz>  archive format (default: tar.gz)
  --version <x.y.z>   override the version read from Cargo.toml
  --skip-build        reuse an existing target/<triple>/release build
  -h, --help          show this help
USAGE
}

while [ $# -gt 0 ]; do
    case "$1" in
        --target) TARGET="${2:?--target needs a value}"; shift 2 ;;
        --label) LABEL="${2:?--label needs a value}"; shift 2 ;;
        --ext) EXT="${2:?--ext needs a value}"; shift 2 ;;
        --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        -h | --help) usage; exit 0 ;;
        *) printf 'dist.sh: unknown argument %s\n' "$1" >&2; usage >&2; exit 64 ;;
    esac
done

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"

# Fail fast, before anything is staged or deleted: GNU tar cannot write zip (it exits 0 and
# silently produces an uncompressed tar named .zip), so a zip needs bsdtar or zip present.
if [ "$EXT" = "zip" ] && ! command -v bsdtar >/dev/null 2>&1 && ! command -v zip >/dev/null 2>&1 &&
   ! tar --version 2>/dev/null | grep -qi bsdtar; then
    printf 'dist.sh: --ext zip needs bsdtar or zip (GNU tar would silently write a tar).\n' >&2
    printf '         use --ext tar.gz, or run tools/dist.ps1 on Windows.\n' >&2
    exit 1
fi

case "$TARGET" in
    *windows*) EXE="scd.exe" ;;
    *) EXE="scd" ;;
esac
if [ -z "$LABEL" ]; then
    case "$TARGET" in
        x86_64-pc-windows-msvc) LABEL="Windows_x64" ;;
        x86_64-unknown-linux-musl) LABEL="Linux_x64_static" ;;
        x86_64-unknown-linux-gnu) LABEL="Linux_x64" ;;
        aarch64-apple-darwin) LABEL="macOS_arm64" ;;
        x86_64-apple-darwin) LABEL="macOS_x64" ;;
        *) LABEL="$TARGET" ;;
    esac
fi
if [ -z "$TARGET" ]; then
    # Only ask rustc when the caller did not name a target, so packaging works on machines
    # that have no Rust toolchain (e.g. unpacking a release archive elsewhere).
    TARGET="$(rustc -vV | sed -n 's|host: ||p')"
fi
if [ -z "$VERSION" ]; then
    VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
fi
[ -n "$VERSION" ] || { echo "dist.sh: cannot read version from Cargo.toml" >&2; exit 1; }

say() { printf '==> %s\n' "$1"; }

if [ "$SKIP_BUILD" -eq 0 ]; then
    say "cargo build --release --locked --target $TARGET"
    cargo build --release --locked --target "$TARGET"
fi
BUILT="target/$TARGET/release/$EXE"
[ -f "$BUILT" ] || { echo "dist.sh: no binary at $BUILT" >&2; exit 1; }

FOLDER="scd-$VERSION-$TARGET"
RELEASE="$REPO/releases"
STAGE_ROOT="$RELEASE/.stage"
STAGE="$STAGE_ROOT/$FOLDER"
rm -rf "$STAGE_ROOT"
mkdir -p "$STAGE/bin" "$STAGE/shell"

say "staging $FOLDER"
cp -f "$BUILT" "$STAGE/bin/$EXE"
chmod +x "$STAGE/bin/$EXE"
cp -f shell/* "$STAGE/shell/"
for file in config.example.json install.ps1 install.cmd install.sh README.md CHANGELOG.md LICENSE; do
    [ -f "$file" ] || { echo "dist.sh: missing $file - a release needs it" >&2; exit 1; }
    cp -f "$file" "$STAGE/"
done
chmod +x "$STAGE/install.sh" "$STAGE/shell/scd.bash" "$STAGE/shell/scd.zsh" "$STAGE/shell/scd.fish" 2>/dev/null || true

PAYLOAD="$RELEASE/$LABEL"
rm -rf "$PAYLOAD"
cp -R "$STAGE" "$PAYLOAD"

ASSET="$FOLDER.$EXT"
rm -f "$RELEASE/$ASSET" "$RELEASE/$ASSET.sha256"
say "creating releases/$ASSET"
( cd "$STAGE_ROOT" && case "$EXT" in
    tar.gz) tar -czf "$RELEASE/$ASSET" "$FOLDER" ;;
    zip)
        # GNU tar CANNOT write zip: `tar -a -cf x.zip` exits 0 and silently writes an
        # uncompressed *tar* named .zip. Only use writers that produce a real zip.
        if command -v bsdtar >/dev/null 2>&1; then
            bsdtar -a -cf "$RELEASE/$ASSET" "$FOLDER"
        elif command -v zip >/dev/null 2>&1; then
            zip -qr "$RELEASE/$ASSET" "$FOLDER"
        elif tar --version 2>/dev/null | grep -qi bsdtar; then
            tar -a -cf "$RELEASE/$ASSET" "$FOLDER"
        else
            echo "dist.sh: refusing to build a .zip here - GNU tar would silently write a tar." >&2
            echo "         install bsdtar or zip, or package with --ext tar.gz." >&2
            exit 1
        fi
        ;;
    *) echo "dist.sh: unsupported --ext $EXT" >&2; exit 64 ;;
esac )

# Trust, then verify: a mislabelled archive is worse than no archive.
magic() { od -An -tx1 -N2 "$1" | tr -d ' \n'; }
case "$EXT" in
    zip)
        if [ "$(magic "$RELEASE/$ASSET")" != "504b" ]; then
            echo "dist.sh: $ASSET has no PK header - it is not a real zip" >&2; exit 1
        fi ;;
    tar.gz)
        if [ "$(magic "$RELEASE/$ASSET")" != "1f8b" ]; then
            echo "dist.sh: $ASSET is not gzip-compressed" >&2; exit 1
        fi ;;
esac

rm -rf "$STAGE_ROOT"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
    else openssl dgst -sha256 -r "$1" | awk '{print $1}'; fi
}
HASH="$(sha256 "$RELEASE/$ASSET")"
printf '%s  %s\n' "$HASH" "$ASSET" >"$RELEASE/$ASSET.sha256"

printf '\n'
printf 'scd %s packaged for %s\n' "$VERSION" "$TARGET"
( cd "$RELEASE" && find "$LABEL" -type f -printf '  %-52p %10s B\n' 2>/dev/null || ls -lR "$LABEL" )
printf '\n'
printf '  upload this file:  releases/%s\n' "$ASSET"
printf '  checksum:          releases/%s.sha256\n' "$ASSET"
printf '  sha256:            %s\n' "$HASH"
