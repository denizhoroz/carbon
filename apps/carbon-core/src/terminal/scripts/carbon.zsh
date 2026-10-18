# Carbon shell integration for zsh. Carbon starts zsh with ZDOTDIR pointing at
# a folder holding this as `.zshrc` (and a `.zshenv` that loads the user's);
# it loads the user's `.zshrc`, then marks prompts and commands with OSC 633.
ZDOTDIR="${CARBON_USER_ZDOTDIR:-$HOME}"
if [ -f "$ZDOTDIR/.zshrc" ]; then . "$ZDOTDIR/.zshrc"; fi

if [ -n "$__CARBON_SHELL_INTEGRATION" ]; then return; fi
__CARBON_SHELL_INTEGRATION=1

__carbon_escape() {
    local value="${1//\\/\\\\}"
    value="${value//;/\\x3b}"
    value="${value//$'\n'/\\x0a}"
    printf '%s' "$value"
}

__carbon_running=0

__carbon_precmd() {
    local code=$?
    if [ "$__carbon_running" = 1 ]; then
        printf '\e]633;D;%s\a' "$code"
    fi
    __carbon_running=0
    printf '\e]633;A\a\e]633;P;Cwd=%s\a' "$(__carbon_escape "$PWD")"
}

__carbon_preexec() {
    __carbon_running=1
    printf '\e]633;E;%s\a\e]633;C\a' "$(__carbon_escape "$1")"
}

autoload -Uz add-zsh-hook
add-zsh-hook precmd __carbon_precmd
add-zsh-hook preexec __carbon_preexec
PS1="${PS1}%{$(printf '\e]633;B\a')%}"
