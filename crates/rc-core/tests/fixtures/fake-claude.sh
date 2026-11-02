#!/bin/sh
# Fake `claude` for rc-core tests. The real binary is never run.
# --fake-busy: print the folder-busy notice and exit.
# default: ask for trust with no newline, read one answer, print the session URL, then stay alive.
case " $* " in
  *" --fake-busy "*)
    echo "Error: this folder is already served by a terminal"
    exit 1
    ;;
esac
printf 'Trust this folder? [y/N] '
read -r answer
echo "Remote Control ready https://claude.ai/code/session_123"
exec sleep 600
