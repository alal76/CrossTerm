#!/usr/bin/env zsh
# CrossTerm Shell Integration — Zsh
# Source this file from .zshrc: source /path/to/crossterm.zsh

__crossterm_osc7() {
  printf '\e]7;file://%s%s\e\\' "${HOST}" "${PWD}"
}

__crossterm_preexec() {
  printf '\e]133;C\e\\'
  __crossterm_cmd_start=${SECONDS}
}

__crossterm_precmd() {
  local exit_code=$?
  local duration=0
  if [[ -n "${__crossterm_cmd_start}" ]]; then
    duration=$(( SECONDS - __crossterm_cmd_start ))
    unset __crossterm_cmd_start
  fi
  printf '\e]133;D;%d\e\\' "${exit_code}"
  printf '\e]133;A\e\\'
  __crossterm_osc7
  printf '\e]7777;duration=%d;exit=%d\e\\' "${duration}" "${exit_code}"
  printf '\e]133;B\e\\'
}

if [[ -z "${__crossterm_installed}" ]]; then
  __crossterm_installed=1
  autoload -Uz add-zsh-hook
  add-zsh-hook preexec __crossterm_preexec
  add-zsh-hook precmd __crossterm_precmd
fi
