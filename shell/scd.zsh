# scd - shell integration for zsh.
#
#   source "$HOME/scd/shell/scd.zsh"      # put this line in ~/.zshrc

_scd_err() {
    if [[ -t 2 && -z "${NO_COLOR:-}" && "${TERM:-}" != dumb ]]; then
        print -u2 -- $'\e[1;31mscd:\e[0m \e[31m'"$*"$'\e[0m'
    else
        print -u2 -- "scd: $*"
    fi
}

scd() {
    emulate -L zsh
    setopt localoptions noerrreturn
    local _scd_bin _scd_tmp _scd_rc _scd_target

    if [[ -n "${SCD_BIN:-}" ]]; then
        _scd_bin="$SCD_BIN"
    else
        # `whence -p` only reports executables on PATH, never this function.
        _scd_bin="${$(whence -p scd 2>/dev/null):-$(whence -p scd.exe 2>/dev/null)}"
    fi
    if [[ -z "$_scd_bin" ]]; then
        _scd_err "cannot find the scd binary - put <install>/bin on PATH, or set SCD_BIN"
        return 127
    fi

    _scd_tmp="$(mktemp "${TMPDIR:-/tmp}/scd.XXXXXXXX")" || return 1
    SCD_TARGET_FILE="$_scd_tmp" "$_scd_bin" "$@"
    _scd_rc=$?

    if (( _scd_rc == 0 )) && [[ -s "$_scd_tmp" ]]; then
        _scd_target="$(head -n 1 "$_scd_tmp")"
        if [[ -n "$_scd_target" && -d "$_scd_target" ]]; then
            cd -- "$_scd_target" || _scd_rc=$?
        fi
    fi
    rm -f -- "$_scd_tmp"
    return "$_scd_rc"
}
