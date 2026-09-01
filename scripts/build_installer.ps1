<#
.SYNOPSIS
    Builds the LiteCast release binary and packages the standalone LiteCast-Setup.exe installer.
#>

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "LiteCast & Kurulum Sihirbazi Derleme Baslatildi" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

$WorkspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $WorkspaceRoot

# Ensure cargo is in PATH
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","User") + ";" + [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";$env:USERPROFILE\.cargo\bin"

# 1. Build main LiteCast application in release mode
Write-Host "`n[1/4] LiteCast ana uygulamasi derleniyor (cargo build --release --bin litecast)..." -ForegroundColor Yellow
cargo build --release --bin litecast

# 2. Stop running instance to avoid file lock and update root LiteCast.exe
Write-Host "[2/4] Kok dizindeki LiteCast.exe guncelleniyor..." -ForegroundColor Yellow
Stop-Process -Name "LiteCast","litecast" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 300
Copy-Item -Path "target\release\litecast.exe" -Destination "LiteCast.exe" -Force

# 3. Build standalone installer & verifier wizard
Write-Host "[3/4] Bagimsiz Kurulum Sihirbazi derleniyor (cargo build --release --bin installer)..." -ForegroundColor Yellow
cargo build --release --bin installer

# 4. Copy to root as LiteCast-Setup.exe
Write-Host "[4/4] LiteCast-Setup.exe paketleniyor..." -ForegroundColor Yellow
Copy-Item -Path "target\release\installer.exe" -Destination "LiteCast-Setup.exe" -Force

$SetupPath = Join-Path $WorkspaceRoot "LiteCast-Setup.exe"
$AppPath = Join-Path $WorkspaceRoot "LiteCast.exe"

Write-Host "`n==================================================" -ForegroundColor Green
Write-Host "DERLEME VE PAKETLEME BASARIYLA TAMAMLANDI!" -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host "Kurulum Paketi: $SetupPath" -ForegroundColor Cyan
Write-Host "Ana Uygulama:    $AppPath" -ForegroundColor Cyan
