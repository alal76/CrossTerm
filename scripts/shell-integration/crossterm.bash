#!/bin/bash
# CrossTerm Shell Integration — Bash
# Source this file from .bashrc: source /path/to/crossterm.bash

# OSC 7: Report CWD to terminal
__crossterm_osc7() {
  printf '\e]7;file://%s%s\e\\' "${HOSTNAME}" "${PWD}"
}

# OSC 133: Command prompt markers + duration tracking
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
  printf '\e]133;D;%s\e\\' "${exit_code}"
  printf '\e]133;A\e\\'
  __crossterm_osc7
  printf '\e]7777;duration=%d;exit=%d\e\\' "${duration}" "${exit_code}"
  printf '\e]133;B\e\\'
}

# Install hooks
if [[ -z "${__crossterm_installed}" ]]; then
  __crossterm_installed=1
  trap '__crossterm_preexec' DEBUG
  PROMPT_COMMAND="__crossterm_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
