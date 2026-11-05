#!/bin/sh
# Fake `claude` for rc-core tests. The real binary is never run.
# Modes (extra args):
#   --fake-busy    print the folder-busy notice and exit.
#   --fake-orphan  start a background `sleep 600`, print the pid to orphan.pid, and exit.
#   --fake-offline   print a DNS error and exit 1, like claude with no network.
#   --fake-reconnect print a DNS error, then connect and print the session URL.
#   --fake-silent-exit  exit 3 with no output.
#   --fake-no-recent  with --continue, print the no-recent-session error and exit 1.
#   (default)      ask the trust question, read one answer, print the session URL,
#                  then ask an unrelated late [y/N] and wait for input.
# Files are written to the working folder: prompt.time and answer.time (epoch seconds),
# answers.log (one line per answer received), argv.log (one line of arguments per run).
echo "$*" >> argv.log
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
case " $* " in
  *" --fake-busy "*)
    echo "Error: this folder is already served by a terminal"
    exit 1
    ;;
  *" --fake-orphan "*)
    sleep 600 &
    echo $! > orphan.pid
    echo "Remote Control ready https://claude.ai/code/session_orphan"
    exit 0
    ;;
  *" --fake-offline "*)
    echo "Error: getaddrinfo ENOTFOUND api.anthropic.com"
    exit 1
    ;;
  *" --fake-reconnect "*)
    echo "Error: getaddrinfo ENOTFOUND api.anthropic.com"
    sleep 1
    echo "Capacity: 1/32"
    echo "Remote Control ready https://claude.ai/code/session_123"
    exec sleep 600
    ;;
  *" --fake-silent-exit "*)
    exit 3
    ;;
  *" --fake-no-recent "*"--continue "*)
    echo "Error: No recent session found in this directory or its worktrees."
    exit 1
    ;;
esac
echo "Quick safety check: is this a project you created or one you trust?"
now > prompt.time
printf 'Trust this folder? [y/N] '
read -r answer
now > answer.time
echo "$answer" >> answers.log
echo "Remote Control ready https://claude.ai/code/session_123"
printf 'Overwrite stale session? [y/N] '
read -r late
echo "$late" >> answers.log
exec sleep 600
