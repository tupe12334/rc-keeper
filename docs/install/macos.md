# Install rc-keeper on macOS

## Requirements

- macOS. The builds are universal, so they run on Apple silicon and Intel.
- Claude Code CLI, installed and logged in.

## Install

Install the desktop app into `/Applications`:

```sh
curl -fsSL https://raw.githubusercontent.com/tupe12334/rc-keeper/main/install.sh | sh
```

Install only the command-line tool into `~/.local/bin`:

```sh
curl -fsSL https://raw.githubusercontent.com/tupe12334/rc-keeper/main/install.sh | sh -s -- --cli
```

Manual install: open the [Releases](https://github.com/tupe12334/rc-keeper/releases) page and download one of these:

- `rc-keeper_<version>_universal.dmg`. Open it and drag `rc-keeper.app` to `/Applications`.
- `rc-keeper.app.tar.gz`. Extract it and move `rc-keeper.app` to `/Applications`.
- `rc-keeper-universal-apple-darwin.tar.gz` for the command-line tool only. Extract it and put `rc-keeper` on your `PATH`.

## First launch

The app is not signed. macOS blocks it the first time.

1. Open `rc-keeper.app`. macOS shows a warning. Close it.
2. Open System Settings, then Privacy & Security.
3. Click "Open Anyway" next to the rc-keeper message. Confirm.

The install script removes the quarantine flag. If you installed by hand, use these steps.

## Launch at login

Open rc-keeper, then turn on "Launch at login" in its settings. The app then starts at login.

To keep folders running at login without the app, install the daemon service. See [Run the daemon at login](#run-the-daemon-at-login).

## Command-line use

The desktop app bundles the `rc-keeper` command. To use it in Terminal, install the command-line tool with `install.sh --cli`. Or add `/Applications/rc-keeper.app/Contents/MacOS` to your `PATH`.

```sh
rc-keeper add ~/projects/my-app
rc-keeper list
rc-keeper status --json
```

All commands are in the [CLI reference](../cli.md).

## Run the daemon at login

```sh
rc-keeper service install
rc-keeper service status
```

On macOS this installs a launchd agent named `io.github.tupe12334.rckeeper.daemon`. It restarts the daemon if it stops. Remove it with `rc-keeper service uninstall`.

## Uninstall

1. Remove the service: `rc-keeper service uninstall`.
2. Delete the app: `rm -rf /Applications/rc-keeper.app`.
3. Delete the command-line tool, if you installed it: `rm ~/.local/bin/rc-keeper`.
4. Optional. Delete your settings: `rm -rf ~/Library/Application\ Support/io.github.tupe12334.rckeeper`.

## Config and logs

- Config: `~/Library/Application Support/io.github.tupe12334.rckeeper`.
- Logs: the daemon keeps the last 500 lines for each folder in memory. Read them with `rc-keeper logs <folder>`.

## Related

- [CLI reference](../cli.md)
- [How it works](../how-it-works.md)
