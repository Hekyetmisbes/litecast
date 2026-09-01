use crate::docs;

pub fn get_help_content() -> &'static str {
    docs::HELP_DOC
}

pub fn get_install_help_content() -> &'static str {
    docs::INSTALL_DOC
}

pub fn get_hotkeys_content() -> &'static str {
    r#"# LiteCast Ultra Güç & Kısayol Kılavuzu

=======================================================
🧮 HESAP MAKİNESİ (INLINE CALCULATOR)
=======================================================
  • 120 * 4 + 20     -> Matematiksel işlem çözer ve sonucu gösterir
  • sqrt(144)        -> Karekök hesaplar
  • 2^8              -> Üs alma
  • 20% of 150       -> Yüzde hesaplama
  • Enter            -> Sonucu panoya (Clipboard) kopyalar

=======================================================
🌐 WEB ARAMA KISAYOLLARI (SEARCH BANGS)
=======================================================
  • g <sorgu>        -> Google'da ara
  • yt <sorgu>       -> YouTube'da ara
  • gh <sorgu>       -> GitHub'da ara
  • wiki <sorgu>     -> Vikipedi'de ara
  • ddg <sorgu>      -> DuckDuckGo'da ara
  • tr <sorgu>       -> Google Çeviri (TR)
  • chat <sorgu>     -> ChatGPT'de sor
  • maps <sorgu>     -> Google Haritalar

=======================================================
📋 PANO GEÇMİŞİ (CLIPBOARD HISTORY)
=======================================================
  • cb / clip / pano -> Son kopyalanan 50 metni listeler
  • cb <kelime>      -> Pano geçmişinde arama yapar
  • Enter            -> Seçili öğeyi panoya kopyalar

=======================================================
⚡ SİSTEM & GÜÇ EYLEMLERİ
=======================================================
  • lock / kilit     -> Windows oturumunu kilitler
  • sleep / uyku     -> Uyku moduna alır
  • restart / reboot -> Bilgisayarı yeniden başlatır
  • shutdown / kapat -> Bilgisayarı kapatır
  • empty bin / çöp  -> Geri Dönüşüm Kutusunu temizler
  • mute / ses       -> Sesi kapatır / açar
  • volup / voldown  -> Ses seviyesini ayarlar
  • ip / ipconfig    -> Yerel IPv4 adresini panoya kopyalar
  • taskmgr / görev  -> Görev Yöneticisi'ni açar

=======================================================
📝 HIZLI SNIPPETLAR (METİN ŞABLONLARI)
=======================================================
  • !date            -> Bugünün tarihi (GG.AA.YYYY)
  • !time            -> Şu anki saat (SS:DD:SN)
  • !datetime        -> Tarih ve saat damgası
  • !iban            -> Tanımlı IBAN numarası
  • !mail            -> Tanımlı e-posta adresi
  • !shrug           -> ¯\_(ツ)_/¯
  • !tableflip       -> (╯°□°)╯︵ ┻━┻
  • !lorem           -> Lorem ipsum paragrafı

=======================================================
🎨 GELİŞTİRİCİ & TASARIMCI ARAÇLARI
=======================================================
  • #3b82f6          -> Renk kutusu önizler, RGB karşılığını verir
  • rgb(37, 99, 235) -> HEX karşılığına çevirir
  • b64 <metin>      -> Base64 kodlar (veya 'b64 d <kod>')
  • uuid / guid      -> Benzersiz UUID v4 üretir
  • sha256 <metin>   -> SHA-256 hash hesaplar
  • len <metin>      -> Karakter ve kelime sayısı sayar

=======================================================
🪟 AÇIK PENCERE YÖNETİCİSİ (WINDOW SWITCHER)
=======================================================
  • win / tab        -> Açık tüm aktif pencereleri listeler
  • win <başlık>     -> Açık pencereler arasında filtreleme yapar
  • Enter            -> Seçilen pencereye anında odaklanır
"#
}
