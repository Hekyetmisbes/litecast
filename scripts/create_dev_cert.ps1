<#
.SYNOPSIS
    Gelistirme ve yerel test ortami icin kendinden imzali (Self-Signed) Kod Imzalama Sertifikasi olusturur.
    Bu sertifika 'scripts/LiteCastDevCert.pfx' olarak kaydedilir ve 'build_installer.ps1' tarafindan otomatik algilanir.
#>

$ErrorActionPreference = "Stop"

$CertSubject = "CN=LiteCast Development, O=LiteCast Team, C=TR"
$CertFile = Join-Path $PSScriptRoot "LiteCastDevCert.pfx"
$CertPassword = ConvertTo-SecureString -String "LiteCastDev123!" -Force -AsPlainText

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "LiteCast Gelistirici Test Sertifikasi Olusturucu" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# 1. Mevcut eski test sertifikalarini temizle
Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $CertSubject } | Remove-Item -Force -ErrorAction SilentlyContinue

# 2. Yeni Code Signing sertifikasi olustur
Write-Host "`n[1/3] Self-Signed Kod Imzalama Sertifikasi uretiliyor..." -ForegroundColor Yellow
$Cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $CertSubject `
    -KeyUsage DigitalSignature `
    -KeyAlgorithm RSA `
    -KeyLength 2048 `
    -HashAlgorithm SHA256 `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddYears(5)

Write-Host "Sertifika Olusturuldu: $($Cert.Thumbprint)" -ForegroundColor Green

# 3. PFX dosyasi olarak disa aktar
Write-Host "[2/3] Sertifika PFX dosyasina aktariliyor ($CertFile)..." -ForegroundColor Yellow
Export-PfxCertificate -Cert $Cert -FilePath $CertFile -Password $CertPassword | Out-Null

# 4. Bilgilendirme
Write-Host "[3/3] Tamamlandi!" -ForegroundColor Green
Write-Host "`nBilgi: 'build_installer.ps1' calistirildiginda bu sertifika otomatik kullanilacaktir." -ForegroundColor Cyan
Write-Host "Test bilgisayarlarinda SmartScreen uyarisi almamak icin bu sertifikayi:" -ForegroundColor Cyan
Write-Host "  'Guvenilen Kok Sertifika Yetkilileri' (Trusted Root Certification Authorities) deposuna yukleyebilirsiniz." -ForegroundColor Yellow
Write-Host "==================================================" -ForegroundColor Cyan
