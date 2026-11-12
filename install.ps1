#requires -Version 5.1
<#
.SYNOPSIS
    Installs next-harness for the current user on Windows.
.DESCRIPTION
    Inside a checkout with a release build, installs your own
    target/release/next-harness.exe. Otherwise downloads the newest release
    (prereleases included) for x86_64-pc-windows-msvc, verifies its SHA-256
    checksum and installs it to %LOCALAPPDATA%\bin, which is added to the
    user PATH. No administrator rights required.

    One-line install:
        irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
#>
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {
    # TLS 1.2 is already the default on current Windows; nothing to do.
}

$Repo = 'SammyYohan/NextHarness'
$Target = 'x86_64-pc-windows-msvc'
$ArchiveName = "next-harness-$Target.zip"
$ApiReleases = "https://api.github.com/repos/$Repo/releases?per_page=20"
$Latest = "https://github.com/$Repo/releases/latest/download"
$InstallDir = Join-Path $env:LOCALAPPDATA 'bin'
$InstallBin = Join-Path $InstallDir 'next-harness.exe'
$TempDir = $null

try {
    $ReleaseBin = $null

    # A checkout with a release build: prefer it, no download needed. When the
    # script is piped through `irm ... | iex`, it has no path and always
    # downloads.
    if ($MyInvocation.MyCommand.Path) {
        $candidate = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'target/release/next-harness.exe'
        if (Test-Path $candidate) {
            $ReleaseBin = $candidate
            Write-Host "Using your local build at $candidate."
        }
    }

    if (-not $ReleaseBin) {
        # The URLs below are the fallback for when the API is unreachable or
        # rate-limited (60 requests/hour unauthenticated).
        $assetUrl = "$Latest/$ArchiveName"
        $checksumUrl = "$assetUrl.sha256"

        # `releases/latest` skips prereleases, and this project ships betas:
        # ask the API for the newest release (prereleases included) that
        # carries both the archive and its checksum, so both files always come
        # from the same release.
        try {
            $releases = @(Invoke-RestMethod -Uri $ApiReleases -UserAgent 'next-harness-installer')
            foreach ($release in $releases) {
                $names = @($release.assets.name)
                if ($names -contains $ArchiveName -and $names -contains "$ArchiveName.sha256") {
                    $assetUrl = ($release.assets | Where-Object { $_.name -eq $ArchiveName }).browser_download_url
                    $checksumUrl = ($release.assets | Where-Object { $_.name -eq "$ArchiveName.sha256" }).browser_download_url
                    break
                }
            }
        } catch {
            # Keep the fallback URLs.
        }

        Write-Host "Downloading the latest release for $Target..."
        $TempDir = Join-Path ([IO.Path]::GetTempPath()) ("next-harness-" + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $TempDir | Out-Null

        $archive = Join-Path $TempDir $ArchiveName
        $checksum = Join-Path $TempDir "$ArchiveName.sha256"

        try {
            Invoke-WebRequest -Uri $assetUrl -OutFile $archive -UseBasicParsing
            Invoke-WebRequest -Uri $checksumUrl -OutFile $checksum -UseBasicParsing
        } catch {
            throw "Download failed. Check that a release exists at https://github.com/$Repo/releases"
        }

        $expected = (Get-Content $checksum -TotalCount 1).Split(' ')[0].Trim().ToLower()
        $actual = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLower()
        if ($expected -ne $actual) {
            throw "SHA-256 mismatch: expected $expected, got $actual"
        }

        Expand-Archive -Path $archive -DestinationPath $TempDir -Force
        $ReleaseBin = Join-Path $TempDir "next-harness-$Target\next-harness.exe"
        if (-not (Test-Path $ReleaseBin)) {
            throw "The archive did not contain the binary."
        }
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path $ReleaseBin -Destination $InstallBin -Force

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not [string]::IsNullOrWhiteSpace($userPath) -and $userPath -like "*$InstallDir*") {
        # Already on the user PATH: nothing to change.
    } else {
        # A blank user PATH must not become ";C:\...\bin": the leading
        # semicolon is harmless but confusing.
        $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        Write-Host "Added $InstallDir to your user PATH."
    }

    $version = (& $InstallBin --version) -join ' '
    Write-Host "Installed: $InstallBin ($version)" -ForegroundColor Green
    Write-Host "Open a new terminal, then run: next-harness --help" -ForegroundColor Cyan
} finally {
    if ($TempDir -and (Test-Path $TempDir)) {
        Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
    }
}
