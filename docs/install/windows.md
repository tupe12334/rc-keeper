# Install rc-keeper on Windows

## Requirements

- Windows 10 or 11, x86_64.
- Claude Code CLI, installed and logged in.

## Install

Open PowerShell and run:

```powershell
irm https://raw.githubusercontent.com/tupe12334/rc-keeper/main/install.ps1 | iex
```

The script downloads the latest setup file and runs it silently. It installs the desktop app only.

Manual install: open the [Releases](https://github.com/tupe12334/rc-keeper/releases) page and download one of these:

- `rc-keeper_<version>_x64-setup.exe`. Run it.
- The `.msi` file. Run it.
- `rc-keeper-x86_64-pc-windows-msvc.zip` for the command-line tool. Extract it and add the folder to your `PATH`.

## First launch

The app is not signed. Windows SmartScreen may block it the first time.

1. Click "More info".
2. Click "Run anyway".

## Launch at login

Open rc-keeper, then turn on "Launch at login" in its settings.

To keep folders running without the desktop app, install the daemon service. See [Run the daemon at login](#run-the-daemon-at-login).

## Command-line use

The zip contains `rc-keeper.exe`. Open a new PowerShell window after you add it to `PATH`.

```powershell
rc-keeper add C:\projects\my-app
rc-keeper list
rc-keeper status --json
```

All commands are in the [CLI reference](../cli.md).

## Run the daemon at login

```powershell
rc-keeper service install
rc-keeper service status
```

On Windows this creates a Task Scheduler task that starts the daemon at logon. Remove it with `rc-keeper service uninstall`.

## Uninstall

1. Remove the service: `rc-keeper service uninstall`.
2. Uninstall the app: open Settings, then Apps, then Installed apps. Select rc-keeper and choose Uninstall.
3. Delete the command-line tool, if you added it to `PATH`. Delete the folder you extracted.
4. Optional. Delete your settings: `%APPDATA%\io.github.tupe12334.rckeeper`.

## Config and logs

- Config: `%APPDATA%\io.github.tupe12334.rckeeper`.
- Logs: the daemon keeps the last 500 lines for each folder in memory. Read them with `rc-keeper logs <folder>`.

## Related

- [CLI reference](../cli.md)
- [How it works](../how-it-works.md)
