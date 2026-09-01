# LiteCast Installation & Development Guide

This guide describes how to set up, build, test, run, and optionally install the LiteCast prototype on Windows from scratch.

---

## 1. System Requirements

- **Operating System:** Windows 10 (version 1809+) or Windows 11 (64-bit x64).
- **Permissions:** Standard user permissions for running and developing LiteCast. Administrator privileges may only be needed when installing build tools or system-wide software.
- **Tools:** Git, Rust toolchain (stable x86_64-pc-windows-msvc), and Microsoft C++ Build Tools / Visual Studio 2022.

---

## 2. Install Development Tools

### Step 2.1: Install Git
Download and install Git from [git-scm.com](https://git-scm.com/) or via Windows Package Manager:
```powershell
winget install --id Git.Git -e --source winget
```

### Step 2.2: Install Visual Studio C++ Build Tools
Install Visual Studio 2022 or Visual Studio Build Tools with the **"Desktop development with C++"** workload:
```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--passive --config vs_config.json"
```

### Step 2.3: Install Rustup and Stable Toolchain
Install `rustup` to manage Rust toolchains:
```powershell
winget install --id Rustlang.Rustup -e --source winget
```
Ensure the default toolchain is configured for 64-bit MSVC:
```powershell
rustup default stable
rustup target add x86_64-pc-windows-msvc
```

### Step 2.4: Verify Installation
Verify that the CLI tools are available in PowerShell:
```powershell
rustc --version
cargo --version
git --version
```

---

## 3. Clone and Build

Clone the repository and enter the project folder:
```powershell
git clone <repository-url>
cd litecast
```

Build the debug development version:
```powershell
cargo build
```

The debug binary is located at `target\debug\litecast.exe`.

---

## 4. Run in Development

Launch LiteCast in development mode:
```powershell
cargo run
```

### Basic Workflow:
- Press **`Alt+Space`** to toggle the launcher window.
- The search field receives focus immediately.
- Type an application name (e.g., `notepad`, `calc`, `code`, `chrome`).
- Use **`Up / Down`** arrow keys to navigate the ranked results.
- Press **`Enter`** to launch the selected application.
- Press **`Escape`** to dismiss the launcher without closing the background process.
- Type **`help`** and press **`Enter`** to open the in-app help system.
- Type **`help install`** and press **`Enter`** to view this complete installation guide offline within LiteCast.
- Type **`reindex`** and press **`Enter`** to refresh installed application indexing.
- Type **`quit`** and press **`Enter`** to terminate the background process.

---

## 5. Quality Checks

Run the automated checks and linter suites before submitting changes:
```powershell
# Format check
cargo fmt --check

# Clippy lints
cargo clippy --all-targets --all-features -- -D warnings

# Automated tests
cargo test
```

---

## 6. Release Build

Create an optimized release build:
```powershell
cargo build --release
```

The optimized release executable is generated at:
```
target\release\litecast.exe
```

To run the release binary directly:
```powershell
.\target\release\litecast.exe
```

---

## 7. Optional Local Installation

You can install LiteCast for your local Windows user profile without needing administrator elevation:

1. Create a local app folder (e.g., `%LOCALAPPDATA%\Programs\LiteCast`).
2. Copy `target\release\litecast.exe` to that folder.
3. (Optional) Create a Start Menu shortcut or add a registry entry to `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` for startup at login.

Example PowerShell setup:
```powershell
$dest = "$env:LOCALAPPDATA\Programs\LiteCast"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item ".\target\release\litecast.exe" -Destination "$dest\litecast.exe" -Force
Write-Host "LiteCast installed to $dest\litecast.exe"
```

---

## 8. Troubleshooting

- **Hotkey Conflict:** If another app (e.g., PowerToys Run) uses `Alt+Space`, LiteCast logs the conflict gracefully and can use alternative activation or configuration.
- **Missing MSVC Linker (link.exe):** Ensure Visual Studio C++ build tools are installed and `x86_64-pc-windows-msvc` toolchain is selected.
- **No Applications Found:** Run the `reindex` command in the launcher or check `%LOCALAPPDATA%\LiteCast\logs\litecast.log`.
- **Display Topology Changes:** If the launcher appears off-screen after detaching monitors, pressing `Alt+Space` re-centers on the currently active monitor cursor.

---

## 9. Uninstall Development Build

1. Close LiteCast via the `quit` command in the search bar or Task Manager.
2. Remove any created startup entry or shortcut.
3. Delete the local installation directory (`%LOCALAPPDATA%\Programs\LiteCast`) and cache folder (`%LOCALAPPDATA%\LiteCast`).
