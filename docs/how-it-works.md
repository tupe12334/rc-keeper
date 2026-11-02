# How it works

rc-keeper starts each `claude remote-control` server in a pseudo-terminal and watches it. If the server exits, the daemon starts it again, with a growing delay between tries.

The daemon owns all folders. The desktop app and the `rc-keeper` command are clients of the daemon. Closing the desktop app does not stop the daemon.

## Rules

- Only one server runs per folder. Stop any terminal `claude rc` in that folder first.
- When Claude asks you to trust the folder, rc-keeper can answer for you. Turn on auto-trust for that folder only.

## Related

- [CLI reference](cli.md)
