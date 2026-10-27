# scd - shell integration for fish.
#
#   source "$HOME/scd/shell/scd.fish"     # or put it in ~/.config/fish/functions/

function scd --description "super cd: jump to a saved directory"
    set -l bin "$SCD_BIN"
    if test -z "$bin"
        # `type -p` reports only external commands, never this function.
        set bin (type -p scd 2>/dev/null; type -p scd.exe 2>/dev/null)
    end
    if test -z "$bin"
        echo "scd: cannot find the scd binary - put <install>/bin on PATH, or set SCD_BIN" >&2
        return 127
    end

    set -l tmp (mktemp)
    env SCD_TARGET_FILE="$tmp" "$bin" $argv
    set -l code $status

    if test $code -eq 0 -a -s "$tmp"
        set -l target (head -n 1 "$tmp")
        if test -n "$target" -a -d "$target"
            cd "$target"; or set code $status
        end
    end
    rm -f "$tmp"
    return $code
end
