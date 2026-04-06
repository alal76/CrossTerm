# CrossTerm Shell Integration — Fish
# Source this file from config.fish: source /path/to/crossterm.fish

function __crossterm_osc7 --on-variable PWD
  printf '\e]7;file://%s%s\e\\' (hostname) "$PWD"
end

set -g __crossterm_cmd_start 0

function __crossterm_preexec --on-event fish_preexec
  printf '\e]133;C\e\\'
  set -g __crossterm_cmd_start (date +%s)
end

function __crossterm_postexec --on-event fish_postexec
  set -l exit_code $status
  set -l duration 0
  if test $__crossterm_cmd_start -gt 0
    set duration (math (date +%s) - $__crossterm_cmd_start)
    set -g __crossterm_cmd_start 0
  end
  printf '\e]133;D;%d\e\\' $exit_code
  printf '\e]133;A\e\\'
  __crossterm_osc7
  printf '\e]7777;duration=%d;exit=%d\e\\' $duration $exit_code
  printf '\e]133;B\e\\'
end
