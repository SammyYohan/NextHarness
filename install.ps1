#requires -Version 5.1
<#
.SYNOPSIS
    Installe next-harness pour l'utilisateur actuel sur Windows.
.DESCRIPTION
    Copie le binaire release compilé dans le dossier utilisateur PATH
    (~\AppData\Local\bin) et ajoute ce dossier au PATH utilisateur de façon
    permanente. Aucune droits administrateur requis.
.NOTES
    Exécuter depuis le dossier contenant target/release/next-harness.exe.
#>
$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$releaseBin = Join-Path $scriptDir "target/release/next-harness.exe"
$installDir = Join-Path $env:LOCALAPPDATA "bin"
$installBin = Join-Path $installDir "next-harness.exe"

if (-not (Test-Path $releaseBin)) {
    Write-Error "Binaire introuvable : $releaseBin (générez-le avec `cargo build --release`)"
}

New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Copy-Item -Path $releaseBin -Destination $installBin -Force

$target = [Environment::GetEnvironmentVariable('Path', 'User'])
if ($target -notlike "*$installDir*") {
    [Environment::SetEnvironmentVariable('Path', "$target;$installDir", 'User')]
}

Write-Host "Installation terminee : $installBin" -ForegroundColor Green
Write-Host "Redemarrez votre terminal, puis tapez : next-harness --help" -ForegroundColor Cyan
