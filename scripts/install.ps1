<#
.SYNOPSIS
    Installs LiteCast and configures it to start with Windows.
#>

$ErrorActionPreference = "Stop"

$InstallDir = "$env:LOCALAPPDATA\Programs\LiteCast"
$BinaryName = "litecast.exe"
$SourcePath = Join-Path $PSScriptRoot "..\target\release\$BinaryName"

if (-not (Test-Path $SourcePath)) {
    Write-Host "Release binary not found at $SourcePath. Building with cargo build --release..." -ForegroundColor Yellow
    Push-Location (Join-Path $PSScriptRoot "..")
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","User") + ";" + [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";$env:USERPROFILE\.cargo\bin"
    cargo build --release
    Pop-Location
}

# Stop old instance gracefully before replacing binary
Get-Process -Name "litecast","LiteCast" -ErrorAction SilentlyContinue | ForEach-Object {
    $_.CloseMainWindow() | Out-Null
    Start-Sleep -Milliseconds 150
    if (-not $_.HasExited) { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
}
Start-Sleep -Milliseconds 200

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$TargetPath = Join-Path $InstallDir $BinaryName
Copy-Item -Path $SourcePath -Destination $TargetPath -Force

# Register in Windows Startup (HKCU Run key)
$RegistryPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
Set-ItemProperty -Path $RegistryPath -Name "LiteCast" -Value "`"$TargetPath`""

Write-Host "LiteCast installed successfully to: $TargetPath" -ForegroundColor Green
Write-Host "Windows başlangıcında otomatik başlama ayarı etkinleştirildi." -ForegroundColor Green

# Start LiteCast in background now
Start-Process -FilePath $TargetPath
Write-Host "LiteCast arka planda çalıştırıldı! Alt+Space ile istediğiniz zaman açabilirsiniz." -ForegroundColor Cyan
