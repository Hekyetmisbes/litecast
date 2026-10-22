fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "LiteCast");
        res.set("FileDescription", "LiteCast Quick Launcher & Search");
        res.set("CompanyName", "LiteCast Team");
        res.set(
            "LegalCopyright",
            "Copyright (C) 2026 LiteCast Team. All rights reserved.",
        );
        res.set("FileVersion", "1.0.1.0");
        res.set("ProductVersion", "1.0.1.0");
        res.set("OriginalFilename", "LiteCast.exe");
        res.set("InternalName", "LiteCast");
        res.set(
            "Comments",
            "Windows için ultra hızlı yerel uygulama başlatıcı ve geliştirici araç takımı.",
        );
        res.set_language(0x0409);
        let _ = res.compile();
    }
}
