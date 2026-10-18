# Carbon shell integration for bash (also Git Bash). Started with
# `bash --init-file <this>`, so it loads the user's startup files first, then
# marks prompts and commands with OSC 633: where each command starts and ends,
# its exit code and the working directory.
if [ -n "$__CARBON_SHELL_INTEGRATION" ]; then return; fi
__CARBON_SHELL_INTEGRATION=1

# Git Bash: the profile sets PATH and the prompt (CHERE_INVOKING keeps the cwd).
if [ -n "$MSYSTEM" ] && [ -f /etc/profile ]; then . /etc/profile; fi
if [ -f "$HOME/.bashrc" ]; then . "$HOME/.bashrc"; fi

__carbon_escape() {
    local value="${1//\\/\\\\}"
    value="${value//;/\\x3b}"
    value="${value//$'\n'/\\x0a}"
    printf '%s' "$value"
}

__carbon_running=0
__carbon_ready=0

__carbon_precmd() {
    local code=$?
    if [ "$__carbon_running" = 1 ]; then
        printf '\e]633;D;%s\a' "$code"
    fi
    __carbon_running=0
    # Git Bash: report the Windows path (C:\...), not the MSYS one (/c/...).
    local cwd="$PWD"
    if [ -n "$MSYSTEM" ] && command -v cygpath >/dev/null 2>&1; then
        cwd="$(cygpath -w "$PWD")"
    fi
    printf '\e]633;A\a\e]633;P;Cwd=%s\a' "$(__carbon_escape "$cwd")"
}

__carbon_preexec() {
    # Only the first command after a prompt counts (not PROMPT_COMMAND's own).
    [ "$__carbon_ready" = 1 ] || return
    [ -n "$COMP_LINE" ] && return
    __carbon_ready=0
    __carbon_running=1
    local line
    line="$(HISTTIMEFORMAT= builtin history 1 | sed 's/^ *[0-9]* *//')"
    printf '\e]633;E;%s\a\e]633;C\a' "$(__carbon_escape "$line")"
}

PROMPT_COMMAND="__carbon_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND};__carbon_ready=1"
PS1="${PS1}\[\e]633;B\a\]"
trap '__carbon_preexec' DEBUG
