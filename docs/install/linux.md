# Install rc-keeper on Linux

## Requirements

- x86_64 or arm64 Linux.
- Claude Code CLI, installed and logged in.

## Install

Install the desktop app:

```sh
curl -fsSL https://raw.githubusercontent.com/tupe12334/rc-keeper/main/install.sh | sh
```

The script picks the desktop app when a display is present. It installs an AppImage to `~/.local/bin/rc-keeper-gui` and adds a desktop entry.

Install only the command-line tool:

```sh
curl -fsSL https://raw.githubusercontent.com/tupe12334/rc-keeper/main/install.sh | sh -s -- --cli
```

Manual install: open the [Releases](https://github.com/tupe12334/rc-keeper/releases) page and download one of these:

| Want | File |
| --- | --- |
| Desktop app, x86_64 | `rc-keeper_<version>_amd64.AppImage` or `rc-keeper_<version>_amd64.deb` |
| Desktop app, arm64 | `rc-keeper_<version>_aarch64.AppImage` or `rc-keeper_<version>_arm64.deb` |
| Command-line tool, x86_64 | `rc-keeper-x86_64-unknown-linux-gnu.tar.gz` |
| Command-line tool, arm64 | `rc-keeper-aarch64-unknown-linux-gnu.tar.gz` |

For an AppImage, run `chmod +x` on the file first. AppImages need FUSE. On Ubuntu, install it with `sudo apt install libfuse2`.

For a `.deb`, install with `sudo apt install ./rc-keeper_<version>_<arch>.deb`.

## First launch

The app is not signed. Your system may warn you the first time. This step is not needed for the command-line tool.

## Launch at login

Open rc-keeper, then turn on "Launch at login" in its settings.

To keep folders running without the desktop app, install the daemon service. See [Run the daemon at login](#run-the-daemon-at-login).

## Command-line use

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

On Linux this installs a systemd user unit. It starts the daemon at login.

systemd user services stop when you log out. To keep them running after logout, enable linger once:

```sh
loginctl enable-linger "$USER"
```

Remove the service with `rc-keeper service uninstall`.

## Servers over SSH

Use the command-line tool on a headless server. You do not need a desktop or a display.

1. Install the command-line tool:

   ```sh
   curl -fsSL https://raw.githubusercontent.com/tupe12334/rc-keeper/main/install.sh | sh -s -- --cli
   ```

   The script selects CLI mode on SSH sessions and on Linux with no display. Pass `--cli` to be explicit. Both x86_64 and arm64 servers work.

2. Add a folder and start the daemon:

   ```sh
   rc-keeper add ~/projects/my-app
   rc-keeper service install
   ```

3. Enable linger so the daemon keeps running after you log out:

   ```sh
   loginctl enable-linger "$USER"
   ```

Check the state at any time with `rc-keeper status --json`.

## Uninstall

1. Remove the service: `rc-keeper service uninstall`.
2. Delete the desktop app files, if you installed them:

   ```sh
   rm ~/.local/bin/rc-keeper-gui ~/.local/share/applications/rc-keeper.desktop
   ```

3. Delete the command-line tool, if you installed it: `rm ~/.local/bin/rc-keeper`.
4. For a `.deb` install, run `sudo apt remove rc-keeper`.
5. If you enabled linger and no longer need it: `loginctl disable-linger "$USER"`.
6. Optional. Delete your settings: `rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/io.github.tupe12334.rckeeper"`.

## Config and logs

- Config: `${XDG_CONFIG_HOME:-~/.config}/io.github.tupe12334.rckeeper`.
- Logs: the daemon keeps the last 500 lines for each folder in memory. Read them with `rc-keeper logs <folder>`.

## Related

- [CLI reference](../cli.md)
- [How it works](../how-it-works.md)
