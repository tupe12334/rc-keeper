# Install rc-keeper from GitHub Releases on Windows.
#
# Usage:
#   irm https://raw.githubusercontent.com/tupe12334/rc-keeper/main/install.ps1 | iex
#
# Environment:
#   RC_KEEPER_VERSION = 'v0.1.0'   install this tag instead of the latest release
#   RC_KEEPER_DRY_RUN = '1'        print the chosen asset URL and stop
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'tupe12334/rc-keeper'
if ($env:RC_KEEPER_VERSION) {
    $api = "https://api.github.com/repos/$repo/releases/tags/$($env:RC_KEEPER_VERSION)"
} else {
    $api = "https://api.github.com/repos/$repo/releases/latest"
}

Write-Host "==> Looking up release from $repo"
try {
    $release = Invoke-RestMethod -Uri $api -Headers @{ Accept = 'application/vnd.github+json' }
} catch {
    $code = [int]$_.Exception.Response.StatusCode
    if ($code -eq 404) {
        if ($env:RC_KEEPER_VERSION) {
            throw "Release $($env:RC_KEEPER_VERSION) not found. See https://github.com/$repo/releases"
        }
        throw "No published release yet for rc-keeper. See https://github.com/$repo/releases"
    }
    if ($code -eq 403) {
        throw 'GitHub API refused the request (HTTP 403). It may be rate limited. Try again later.'
    }
    throw "GitHub API request failed: $($_.Exception.Message)"
}

# Prefer the NSIS setup (silent install with /S), then the MSI.
$asset = $release.assets | Where-Object { $_.name -like '*-setup.exe' } | Select-Object -First 1
if (-not $asset) {
    $asset = $release.assets | Where-Object { $_.name -like '*.msi' } | Select-Object -First 1
}
if (-not $asset) {
    throw "Release $($release.tag_name) has no Windows asset (*-setup.exe or *.msi)."
}

Write-Host "==> Release: $($release.tag_name)"
Write-Host "==> Asset: $($asset.browser_download_url)"
if ($env:RC_KEEPER_DRY_RUN -eq '1') {
    Write-Output $asset.browser_download_url
    return
}

$file = Join-Path $env:TEMP $asset.name
Write-Host '==> Downloading'
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $file -UseBasicParsing

Write-Host '==> Installing (a window may appear; you may be asked for permission)'
try {
    if ($file -like '*.msi') {
        $proc = Start-Process -FilePath 'msiexec.exe' -ArgumentList "/i `"$file`" /passive /norestart" -Wait -PassThru
    } else {
        $proc = Start-Process -FilePath $file -ArgumentList '/S' -Wait -PassThru
    }
    if ($proc.ExitCode -ne 0) {
        throw "Installer exited with code $($proc.ExitCode). Run $file manually to see details."
    }
} finally {
    Remove-Item -Path $file -ErrorAction SilentlyContinue
}

Write-Host '==> Installed rc-keeper. Start it from the Start menu.'
Write-Host 'If Windows SmartScreen blocks the app, choose "More info", then "Run anyway".'
