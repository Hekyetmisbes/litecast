# LiteCast

A lightweight, fast, keyboard-first native Windows application launcher built in Rust.

LiteCast is engineered to provide an instant launcher experience inspired by modern launcher workflows while avoiding heavy web-based runtimes (Electron, Chromium, WebView2, Tauri) to maintain extremely low idle memory consumption (< 40 MB) and zero background CPU usage.

---

## Key Features

- **Global Hotkey (`Alt+Space`):** Instantly toggle the launcher window anywhere on Windows.
- **Sub-50ms Responsiveness:** Focused search input immediately upon display.
- **Fast Fuzzy Search:** Subsequence and prefix matching with deterministic ranking.
- **Native Windows Indexing:** Automatically discovers user and system Start Menu applications without crawling entire drives.
- **Embedded Offline Help & Guides:**
  - `help` opens the in-app help overview.
  - `help install` opens the embedded [`docs/INSTALL.md`](docs/INSTALL.md) guide offline.
- **Lightweight Native UI:** Direct Win32 rendering and window management for minimal footprint.
- **Local & Private:** 100% offline, zero network telemetry.

---

## Quick Start

### Prerequisites
- Windows 10 / 11 x64
- Rust (stable toolchain)
- Visual Studio C++ Build Tools

### Build & Run
```powershell
# Build debug binary
cargo build

# Run application
cargo run
```

For full installation and deployment details, see [`docs/INSTALL.md`](docs/INSTALL.md).

---

## Architecture Overview

```
src/
├── main.rs              # Minimal process entry point
├── app.rs               # Application lifecycle and event orchestration
├── platform_windows.rs  # Win32 APIs: Hotkeys, ShellExecute, window positioning
├── ui.rs                # Native launcher window, controls, and help views
├── indexer.rs           # Start menu discovery and local cache
├── search.rs            # Fuzzy search and ranking engine
├── commands/            # Internal command system (help, reindex, quit)
│   ├── mod.rs
│   └── help.rs
├── docs.rs              # Compile-time embedded documentation (include_str!)
├── history.rs           # Recent usage ranking persistence
└── telemetry_local.rs   # Local diagnostic logging
```

---

## Development & Quality Checks

```powershell
# Check code formatting
cargo fmt --check

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Run automated tests
cargo test
```

---

## License
MIT / Apache 2.0
