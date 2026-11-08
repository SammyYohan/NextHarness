#requires -Version 5.1
<#
.SYNOPSIS
    Installs next-harness for the current user on Windows.
.DESCRIPTION
    Inside a checkout with a release build, installs your own
    target/release/next-harness.exe. Otherwise downloads the latest release
    for x86_64-pc-windows-msvc, verifies its SHA-256 checksum and installs it
    to %LOCALAPPDATA%\bin, which is added to the user PATH. No administrator
    rights required.

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
        Write-Host "Downloading the latest release for $Target..."
        $TempDir = Join-Path ([IO.Path]::GetTempPath()) ("next-harness-" + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $TempDir | Out-Null

        $archive = Join-Path $TempDir "next-harness-$Target.zip"
        $checksum = Join-Path $TempDir "next-harness-$Target.zip.sha256"

        try {
            Invoke-WebRequest -Uri "$Latest/next-harness-$Target.zip" -OutFile $archive -UseBasicParsing
            Invoke-WebRequest -Uri "$Latest/next-harness-$Target.zip.sha256" -OutFile $checksum -UseBasicParsing
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
    if ($userPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
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
