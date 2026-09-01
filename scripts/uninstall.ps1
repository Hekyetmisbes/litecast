<#
.SYNOPSIS
    Uninstalls LiteCast for the current user profile.
#>

$ErrorActionPreference = "SilentlyContinue"

# Stop any running instances of litecast
Stop-Process -Name "litecast" -Force -ErrorAction SilentlyContinue

$InstallDir = "$env:LOCALAPPDATA\Programs\LiteCast"
$DataDir = "$env:LOCALAPPDATA\LiteCast"

if (Test-Path $InstallDir) {
    Remove-Item -Path $InstallDir -Recurse -Force
    Write-Host "Removed application files at $InstallDir" -ForegroundColor Green
}

if (Test-Path $DataDir) {
    Remove-Item -Path $DataDir -Recurse -Force
    Write-Host "Removed cache and data files at $DataDir" -ForegroundColor Green
}

Write-Host "LiteCast uninstalled cleanly." -ForegroundColor Green
