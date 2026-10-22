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

# Helper function to sign a binary with RFC3161 timestamp
function Sign-Binary($filePath, $cert) {
    if ($cert -and (Test-Path $filePath)) {
        Write-Host "  -> $filePath imzalanıyor (SHA-256 + RFC3161 Timestamp)..." -ForegroundColor Yellow
        $sig = Set-AuthenticodeSignature -FilePath $filePath -Certificate $cert -HashAlgorithm SHA256 -TimestampServer "http://timestamp.digicert.com" -ErrorAction SilentlyContinue
        if ($sig.Status -eq "Valid" -or $sig.Status -eq "UnknownError") {
            Write-Host "  [OK] $filePath basariyla imzalandi." -ForegroundColor Green
        } else {
            Write-Host "  [Uyarı] $filePath imza durumu: $($sig.Status)" -ForegroundColor DarkYellow
        }
    }
}

# 1. Load Certificate if available
$DevCertFile = Join-Path $PSScriptRoot "LiteCastDevCert.pfx"
$CertPath = if ($env:CERT_PATH -and (Test-Path $env:CERT_PATH)) { $env:CERT_PATH } elseif (Test-Path $DevCertFile) { $DevCertFile } else { $null }
$LoadedCert = $null
if ($CertPath) {
    try {
        $PlainPassword = if ($env:CERT_PASSWORD) { $env:CERT_PASSWORD } else { "LiteCastDev123!" }
        $ResolvedCertPath = (Resolve-Path $CertPath).Path
        $LoadedCert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($ResolvedCertPath, $PlainPassword)
        Write-Host "  -> Sertifika bulundu: $($LoadedCert.Subject)" -ForegroundColor Green
    } catch {
        Write-Host "  [Uyari] Sertifika yuklenemedi: $_" -ForegroundColor DarkYellow
    }
}

# 2. Build main LiteCast application in release mode
Write-Host "`n[1/5] LiteCast ana uygulamasi derleniyor (cargo build --release --bin litecast)..." -ForegroundColor Yellow
cargo build --release --bin litecast

# 3. Stop running instance gracefully, then update root LiteCast.exe
Write-Host "[2/5] Kok dizindeki LiteCast.exe guncelleniyor..." -ForegroundColor Yellow
Get-Process -Name "LiteCast","litecast" -ErrorAction SilentlyContinue | ForEach-Object {
    if ($_.Path -and ($_.Path -like "*$WorkspaceRoot*")) {
        $_.CloseMainWindow() | Out-Null
        Start-Sleep -Milliseconds 150
        if (-not $_.HasExited) { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
    }
}
Start-Sleep -Milliseconds 200
Copy-Item -Path "target\release\litecast.exe" -Destination "LiteCast.exe" -Force

# 4. SIGN LiteCast.exe FIRST (so embedded payload inside installer is already signed!)
$AppPath = Join-Path $WorkspaceRoot "LiteCast.exe"
if ($LoadedCert) {
    Write-Host "[3/5] LiteCast.exe imzalanıyor (ic binary)..." -ForegroundColor Yellow
    Sign-Binary $AppPath $LoadedCert
} else {
    Write-Host "[3/5] Imza sertifikasi yok, imzasiz devam ediliyor." -ForegroundColor DarkGray
}

# 5. Build standalone installer & verifier wizard (embeds signed LiteCast.exe)
Write-Host "[4/5] Bagimsiz Kurulum Sihirbazi derleniyor (cargo build --release --bin installer)..." -ForegroundColor Yellow
cargo build --release --bin installer

# 6. Copy to root as LiteCast-Setup.exe and sign the installer itself
Write-Host "[5/5] LiteCast-Setup.exe paketleniyor ve imzalanıyor..." -ForegroundColor Yellow
Copy-Item -Path "target\release\installer.exe" -Destination "LiteCast-Setup.exe" -Force
$SetupPath = Join-Path $WorkspaceRoot "LiteCast-Setup.exe"

if ($LoadedCert) {
    Sign-Binary $SetupPath $LoadedCert
} else {
    Write-Host "  [Bilgi] Kod imzalama sertifikasi bulunamadi (imzasiz derlendi)." -ForegroundColor DarkGray
    Write-Host "  -> Test imzalama yapmak icin: 'powershell scripts/create_dev_cert.ps1'" -ForegroundColor DarkGray
    Write-Host "  -> Resmi sertifikanizi 'scripts/LiteCastDevCert.pfx' yoluna koyabilir veya CERT_PATH ortam degiskenini ayarlayabilirsiniz." -ForegroundColor DarkGray
}

Write-Host "`n==================================================" -ForegroundColor Green
Write-Host "DERLEME VE PAKETLEME BASARIYLA TAMAMLANDI!" -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host "Kurulum Paketi: $SetupPath" -ForegroundColor Cyan
Write-Host "Ana Uygulama:    $AppPath" -ForegroundColor Cyan

