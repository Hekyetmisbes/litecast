# LiteCast — Windows İçin Ultra Hafif ve Hızlı Uygulama Başlatıcı

> **Gereksiz yüklerden arındırılmış, saf Rust ve yerel Win32 API'leri ile geliştirilmiş, klavye odaklı yeni nesil Windows Launcher.**

[![Platform: Windows 10/11](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-0078D6?logo=windows&logoColor=white)](#)
[![Built with: Rust](https://img.shields.io/badge/Built%20With-Rust%202024-CE412B?logo=rust&logoColor=white)](#)
[![Memory: < 40MB](https://img.shields.io/badge/RAM-~25--40%20MB-success)](#)
[![Response: < 50ms](https://img.shields.io/badge/Response-%3C%2050ms-brightgreen)](#)
[![License: MIT / Apache 2.0](https://img.shields.io/badge/License-MIT%20%2F%20Apache%202.0-blue)](#)
[![Version: 1.0.1](https://img.shields.io/badge/Version-1.0.1-purple)](#)

---

## 📌 Proje Özeti

**LiteCast**, macOS'taki Spotlight ve Raycast deneyimini Windows ekosistemine taşıyan, ancak **Electron, Chromium, WebView2 veya Tauri gibi ağır web motorlarını tamamen reddeden** yerel (native) bir üretkenlik ve uygulama başlatıcı aracıdır.

Günümüz masaüstü araçlarının onlarca megabaytlık web motorları ve yüzlerce megabayt RAM tüketerek sistemi yavaşlatmasına bir tepki olarak, doğrudan **Rust** ve saf **Win32 / DWM (Desktop Window Manager)** API'leri kullanılarak sıfırdan inşa edilmiştir.

* **0% Boşta CPU Tüketimi:** Arka planda çalışırken işlemcinizi asla meşgul etmez.
* **Düşük Bellek Ayak İzi:** Yalnızca ~25 - 40 MB RAM tüketimi ile arka planda sessizce bekler.
* **< 50ms Anında Tepki:** `Alt + Space` tuşuna bastığınız an klavye odağı hazır olarak ekrana gelir.
* **%100 Çevrimdışı & Gizlilik Odaklı:** Sıfır telemetri, sıfır veri toplama; tüm işlemler tamamen yerel donanımınızda gerçekleşir.

---

## 🚀 Öne Çıkan Özellikler

### ⚡ 1. Işık Hızında Uygulama & İçerik Arama (Fuzzy Search)
* **Akıllı İndeksleme:** Başlat Menüsü, Masaüstü kısayolları, Windows Store (UWP) uygulamaları, Chrome/Edge PWA'ları ve Denetim Masası araçlarını disk taraması yapmadan milisaniyeler içinde indeksler.
* **Hata Toleranslı Arama (Fuzzy Matching):** Yazım hatalarını tolere eden ve kullanım sıklığınıza göre öğrenip üst sıralara taşıyan akıllı sıralama algoritması.
* **Hızlı Kısaltmalar (Aliases):** `yt` (YouTube), `sp` (Spotify), `dc` (Discord), `st` (Steam), `gh` (GitHub), `calc` (Hesap Makinesi), `cmd`, `ps` (PowerShell) gibi kısaltmalarla tek hamlede erişim.

### 🛠️ 2. Entegre Geliştirici & Tasarımcı Araçları
Arayüzden çıkmadan veya harici siteler açmadan doğrudan arama kutusuna yazarak çalıştırabileceğiniz araçlar:
* **Base64:** `b64 encode <metin>` / `b64 decode <metin>`
* **Hash Üretici:** `hash <metin>` (MD5 ve SHA-256 anında hesaplanır)
* **UUID / GUID v4:** `uuid` yazarak tek tıkla panoya kopyalanabilir benzersiz kimlik üretme
* **Renk Dönüştürücü & Önizleme:** HEX (`#3498db`) veya RGB (`rgb(52, 152, 219)`) girdiğinizde renk kutucuğu ve farklı formatlardaki karşılıkları
* **JSON Biçimlendirici:** `json <tek-satır-json>` ile düzenli / sıkıştırılmış JSON çıktısı
* **Metin Dönüştürücüler:** `camelCase`, `snake_case`, `kebab-case`, `UPPERCASE`, `lowercase`, `len <metin>` (kelime ve karakter sayısı)
* **URL & Timestamp:** URL Encode/Decode, Unix Epoch zaman damgası çevirici

### 🧮 3. Hesap Makinesi & Akıllı Dönüştürücüler
* **Matematik Motoru:** `(45 * 12) / 3 + sqrt(144)`, `%18 of 2500` gibi karmaşık veya pratik hesaplamaları doğrudan arama çubuğuna yazın.
* **Birim Dönüştürücü:** Uzunluk (km, m, ft, in, mil), Ağırlık (kg, lb, g), Sıcaklık (°C, °F) ve Veri Boyutu (MB, GB, TB) dönüşümleri (örneğin: `150 km to miles`, `16 gb to mb`).
* **Döviz & Kripto:** Güncel kurlar üzerinden hızlı para birimi çevirileri (`100 usd to try`, `50 eur to usd`, `0.5 btc to usd`).
* **Dünya Saatleri:** `time in tokyo`, `time in london`, `time in new york` ile anlık yerel saatleri öğrenme.

### 📋 4. Pano Geçmişi (Clipboard Manager) & Hızlı Şablonlar (Snippets)
* Kopyaladığınız metinleri hafızada tutar, geçmişteki kopyalara kolayca erişip tekrar yapıştırmanızı sağlar.
* Sık kullandığınız metin kalıplarını (e-posta taslakları, kod blokları, adresler) snippet olarak kaydedip anında çağırabilirsiniz.

### 🪟 5. Açık Pencere Yöneticisi (Window Switcher)
* Klavyeden elinizi kaldırmadan o an çalışan tüm açık pencereleri listeleyin, başlığına göre filtreleyin ve anında o pencereye odaklanın (Alt+Tab yerine modern bir alternatif).

### 📁 6. Hızlı Dosya Sistemi Gezgini (File Browser)
* `C:\Users\...` şeklinde doğrudan dosya yolu yazarak klasörler arasında klavye oklarıyla hızlıca gezinin, dosyaları varsayılan uygulamalarıyla açın.

### 💻 7. Terminal & Komut Koşturucu
* `> dir` veya `> git status` gibi komutlarla PowerShell / CMD pencerelerini doğrudan girdiğiniz komutla başlatın.

### ⚙️ 8. Windows Sistem & Güç Eylemleri
* Tek satır komutla sistemi yönetin: `Kilitle`, `Uyku Modu`, `Yeniden Başlat`, `Kapat`, `Çöp Kutusunu Boşalt`, `Sesi Aç / Kıs / Kapat`.

### 🎨 9. Modern Win32 & Windows 11 DWM Tasarımı
* Windows 11'in yerel **Mica / Acrylic** arka plan bulanıklığı ve modern karanlık tema (Dark Theme) desteği.
* Yüksek DPI (HiDPI) ve çoklu monitör kurulumlarında keskin piksel netliği.

### 🔄 10. Yerel Güncelleyici & Ayarlar Paneli
* GitHub Releases üzerinden yeni sürümleri otomatik denetleme ve tek tıkla güncelleme.
* Başlangıçta çalıştırma (Autostart) ve tema ayarlarını yönetebileceğiniz görsel ayarlar ekranı.

---

## 📊 Performans & Kaynak Tüketimi Karşılaştırması

LiteCast, özellikle web tabanlı hantal araçlara kıyasla olağanüstü kaynak verimliliği sağlar:

| Kriter | **LiteCast (Rust)** | Raycast (macOS) / PowerToys Run | Electron / Web Tabanlı Başlatıcılar |
| :--- | :---: | :---: | :---: |
| **Geliştirme Teknolojisi** | **Saf Rust + Win32 GDI/DWM** | C# / WPF / Native | JavaScript / HTML / Electron / Chromium |
| **Boşta RAM Tüketimi** | **~25 – 40 MB** | ~150 – 350 MB | ~300 – 700 MB |
| **Boşta CPU Tüketimi** | **%0.0 (Tamamen Uyku Modu)** | %0.5 – %2.0 | %1.0 – %5.0 |
| **Tetiklenme Gecikmesi** | **< 50 ms (Anında)** | ~100 – 200 ms | ~300 – 800 ms |
| **Kurulum / Binary Boyutu** | **~1.5 MB (Tek Exe)** | ~150 – 300 MB | ~100 – 250 MB |
| **İnternet / Telemetri İhtiyacı** | **%0 Çevrimdışı (Sıfır İzleme)** | Kısmi Bulut / Telemetri | Çoğunlukla Çevrimiçi / Telemetri |

---

## ⌨️ Temel Kısayollar ve Kullanım

| Kısayol | İşlev |
| :--- | :--- |
| **`Alt + Space`** | LiteCast arayüzünü her yerden açar veya gizler. |
| **`Enter`** | Seçili uygulamayı / işlemi çalıştırır veya kopyalar. |
| **`Yukarı / Aşağı Ok`** | Arama sonuçları arasında gezinir. |
| **`PageUp / PageDown`** | Yardım ve uzun belgelerde sayfa kaydırır. |
| **`Esc`** | Arayüzü kapatır veya önceki ekrana döner. |

### Hızlı Alias Örnekleri
* `yt` ➔ YouTube
* `sp` ➔ Spotify
* `dc` ➔ Discord
* `st` ➔ Steam
* `wa` ➔ WhatsApp
* `gh` ➔ GitHub
* `calc` ➔ Hesap Makinesi
* `code` ➔ Visual Studio Code
* `cmd` / `ps` ➔ Komut Satırı / PowerShell

### Dahili Komutlar
* `help` : Uygulama içi yardım merkezini açar.
* `autostart` : Windows ile birlikte otomatik başlatmayı açar/kapatır.
* `reindex` : Uygulama ve kısayol indeksini anında yeniler.
* `quit` : LiteCast uygulamasından tamamen çıkar.

---

## 🏗️ Mimari & Teknoloji Yığını

* **Programlama Dili:** Rust (2024 Edition)
* **Windows Entegrasyonu:** `windows-rs` (0.62+)
* **Grafik & Pencereleme:** Saf Win32 API (`GDI`, `DWM`, `HiDpi`)
* **Arama & Sıralama:** Deterministik Subsequence & Prefix Fuzzy Matching Engine
* **Serileştirme:** `serde`, `serde_json`
* **Dağıtım:** Bağımsız tekil Portable EXE (`LiteCast.exe`) ve Yerel Kurucu (`LiteCast-Setup.exe`)

---

## 📥 İndirme & Kurulum

### Sistem Gereksinimleri
* **İşletim Sistemi:** Windows 10 (64-bit) veya Windows 11 (64-bit)
* **İşlemci:** x86_64 mimarili herhangi bir işlemci
* **RAM:** 50 MB boş bellek

### Seçenek 1: Kurulum Dosyası (Önerilen)
1. **[LiteCast-Setup.exe](https://github.com/Hekyetmisbes/litecast/releases/download/v1.0.1/LiteCast-Setup.exe)** dosyasını indirin.
2. Kurulumu tamamlayın; LiteCast sistem tepsisine yerleşecek ve `Alt + Space` ile anında kullanıma hazır olacaktır.

### Seçenek 2: Taşınabilir Sürüm (Portable)
1. **[LiteCast.exe](https://github.com/Hekyetmisbes/litecast/releases/download/v1.0.1/LiteCast.exe)** dosyasını indirin.
2. İstediğiniz bir klasöre yerleştirip doğrudan çalıştırın.

### Seçenek 3: Kaynak Koddan Derleme
```powershell
# Depoyu klonlayın
git clone https://github.com/Hekyetmisbes/litecast.git
cd litecast

# Optimize edilmiş release sürümünü derleyin
cargo build --release

# Uygulamayı başlatın
cargo run --release
```

---

## 📄 Lisans
Bu proje **MIT** ve **Apache 2.0** lisansları altında açık kaynak olarak geliştirilmektedir.
