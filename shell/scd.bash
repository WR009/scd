# scd - shell integration for bash.
#
#   source "$HOME/scd/shell/scd.bash"     # put this line in ~/.bashrc
#
# scd resolves the directory and writes it to $SCD_TARGET_FILE; this function does the cd,
# because a child process cannot change its parent shell's working directory.

scd() {
    local _scd_bin _scd_tmp _scd_rc _scd_target

    if [ -n "${SCD_BIN:-}" ]; then
        _scd_bin="$SCD_BIN"
    else
        # `type -P` only looks at executables on PATH, never at this function.
        _scd_bin="$(type -P scd 2>/dev/null || type -P scd.exe 2>/dev/null)"
    fi
    if [ -z "$_scd_bin" ]; then
        echo "scd: cannot find the scd binary - put <install>/bin on PATH, or set SCD_BIN" >&2
        return 127
    fi

    _scd_tmp="$(mktemp "${TMPDIR:-/tmp}/scd.XXXXXXXX")" || return 1
    SCD_TARGET_FILE="$_scd_tmp" "$_scd_bin" "$@"
    _scd_rc=$?

    if [ "$_scd_rc" -eq 0 ] && [ -s "$_scd_tmp" ]; then
        _scd_target="$(head -n 1 "$_scd_tmp")"
        if [ -n "$_scd_target" ] && [ -d "$_scd_target" ]; then
            cd -- "$_scd_target" || _scd_rc=$?
        fi
    fi
    rm -f -- "$_scd_tmp"
    return "$_scd_rc"
}
