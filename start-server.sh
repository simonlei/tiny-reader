#!/usr/bin/env bash
# ===========================================================================
#  Tiny Reader - start the data server (Git Bash / MSYS2)
#
#  Usage:  ./start-server.sh
#
#  Bash twin of start-server.bat. Output stays ASCII-only so it also renders
#  correctly when the script is invoked from cmd.exe.
# ===========================================================================
set -uo pipefail

cd "$(dirname "$0")" || exit 1

log() { echo "[tiny-reader] $*"; }

# ------------------------------------------------------------- locate MSVC
VCVARS=""
for base in "/c/Program Files (x86)/Microsoft Visual Studio/2022" \
            "/c/Program Files/Microsoft Visual Studio/2022"; do
    for ed in BuildTools Community Professional Enterprise; do
        p="$base/$ed/VC/Auxiliary/Build/vcvarsall.bat"
        [ -f "$p" ] && VCVARS="$p"
    done
done

# Run the given command line with the MSVC x64 environment in place.
#   1) preferred: cmd.exe + vcvarsall.bat
#   2) fallback : source tools/msvc-env.sh (used when cmd.exe is blocked)
#
# NOTE: the command cannot be passed as `cmd.exe /c "call \"...vcvarsall\" x64 && ..."`.
# MSYS escapes the inner quotes to \" when spawning native programs, and cmd.exe
# does not understand that escaping (it then reports:
#   '\"C:\...\vcvarsall.bat\"' is not recognized ...).
# Writing a throwaway .bat and handing cmd.exe just its path avoids nested quotes.
#
# The .bat also has to re-export PATH in Windows form: cmd.exe inherits MSYS's
# POSIX-style PATH (/c/Users/.../.cargo/bin), which cmd cannot resolve, so
# nothing but the absolute vcvarsall path would be found.
run_msvc() {
    local cmdline="$*"

    if [ -n "$VCVARS" ] && [ "${MSVC_NO_CMD:-0}" != "1" ] \
       && MSYS_NO_PATHCONV=1 cmd.exe /c "ver" >/dev/null 2>&1; then
        local bat bat_win rc win_path
        bat="$(mktemp "${TMPDIR:-/tmp}/tiny-reader-msvc-XXXXXX.bat")" || bat=""
        if [ -n "$bat" ]; then
            win_path="$(cygpath -w -p "$PATH" 2>/dev/null || true)"
            {
                printf '@echo off\r\n'
                if [ -n "$win_path" ]; then
                    printf 'set "PATH=%s;%%PATH%%"\r\n' "$win_path"
                fi
                printf 'call "%s" x64\r\n' "$(cygpath -w "$VCVARS")"
                printf '%s\r\n' "$cmdline"
            } > "$bat"
            bat_win="$(cygpath -w "$bat")"
            MSYS_NO_PATHCONV=1 cmd.exe /c "$bat_win"
            rc=$?
            rm -f "$bat"
            return $rc
        fi
    fi

    if [ -f tools/msvc-env.sh ]; then
        log "cmd.exe unavailable - falling back to tools/msvc-env.sh"
        MSVC_QUIET=1 . ./tools/msvc-env.sh \
            || log "WARNING: MSVC environment not loaded."
    elif [ -z "$VCVARS" ]; then
        log "WARNING: vcvarsall.bat not found."
        log "  If the build fails, run from an \"x64 Native Tools Command Prompt\"."
    fi

    eval "$cmdline"
}

if [ ! -f server/config.toml ]; then
    log "No server/config.toml yet - it will be generated on first run."
fi

log "Starting server on http://127.0.0.1:8787"
log "Press Ctrl+C to stop."
echo

run_msvc "cargo run --manifest-path server/Cargo.toml"
rc=$?
if [ "$rc" -ne 0 ]; then
    log "Server exited with code $rc"
    [ "${MSVC_NO_CMD:-0}" != "1" ] && log "  If it died on MSVC headers/libs, retry with: MSVC_NO_CMD=1 ./start-server.sh"
fi
exit 0
