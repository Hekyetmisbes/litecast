# LiteCast In-App Help & Reference

Welcome to LiteCast — a lightweight, fast, native Windows launcher designed with minimal memory footprint and zero idle CPU usage.

---

## 1. Getting Started

- Toggle the launcher anywhere by pressing **`Alt+Space`**.
- Immediately start typing to search for installed Windows applications.
- Press **`Enter`** to launch the top result or selected item.
- Press **`Escape`** to dismiss the launcher to the background.

---

## 2. Quick Search Aliases (Hızlı Kısaltmalar)

You can type any of these short aliases directly in the search bar and press **Enter**:

| Alias | Target Application |
| :--- | :--- |
| **`yt`** | YouTube (Chrome App / Web) |
| **`sp`** | Spotify |
| **`st`** | Steam |
| **`dc`** | Discord |
| **`wa`** | WhatsApp |
| **`gh`** | GitHub |
| **`cp`** / **`cm`** | Denetim Masası (Control Panel) |
| **`calc`** | Hesap Makinesi (Calculator) |
| **`np`** | Not Defteri (Notepad) |
| **`ps`** | Windows PowerShell |
| **`cmd`** | Komut İstemi (Command Prompt) |
| **`code`** | Visual Studio Code |

---

## 3. Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| **`Alt+Space`** | Show or hide the launcher |
| **`Up` / `Down`** | Navigate through search results / scroll help |
| **`PageUp` / `PageDown`** | Scroll pages inside Help and Install guides |
| **`Enter`** | Launch selected application or execute typed command |
| **`Escape`** | Close / hide launcher or return from Help view |

---

## 4. Internal Commands

Type any of the following commands into the search box:

- **`help`**: Opens this in-app Help and reference guide.
- **`hotkey`** / **`shortcuts`** / **`aliases`**: Displays the quick alias list reference.
- **`autostart`** / **`startup`**: Toggles automatic launch at Windows startup on/off.
- **`help install`**: Displays the full offline installation and setup guide (`docs/INSTALL.md`).
- **`reindex`**: Scans the Start Menu and rebuilds the local application index cache.
- **`quit`**: Closes LiteCast and terminates the background process cleanly.

---

## 5. Application Search & Ranking

LiteCast discovers installed applications across:
- User and System Start Menu Programs (including Chrome Apps/PWAs)
- Windows Store / UWP Applications (`%LOCALAPPDATA%\Microsoft\WindowsApps`)
- User and Public Desktop Shortcuts
- Built-in Windows System Tools & Settings (Control Panel, Settings, Task Manager, etc.)

---

## 6. Installation Guide

To read the complete installation, compilation, and setup instructions without an internet connection, type:
```
help install
```
and press **Enter**.
