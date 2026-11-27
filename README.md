# shotify

<p align="center">
  <img src="./ui/shotstylelogo.svg" width="160" alt="shotify logo">
</p>

<p align="center">
  <b>Screenshots, cleanly framed and annotated.</b><br>
  Open-source desktop app for Linux, macOS, and Windows.
</p>

<p align="center">
  <a href="https://lalitmukesh69.github.io/shotify/"><img src="https://img.shields.io/badge/Website-GitHub%20Pages-000000?style=flat-square&logo=github" alt="Website" /></a>
  <img src="https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-111827?style=flat-square" alt="Platforms" />
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0-2563eb?style=flat-square" alt="GPL-3.0 License" /></a>
  <a href="https://github.com/Lalitmukesh69/shotify/releases/latest"><img src="https://img.shields.io/github/v/release/Lalitmukesh69/shotify?style=flat-square&color=10b981" alt="Latest Release" /></a>
</p>

<p align="center">
  <img src="./docs/media/shotdemo.gif" width="760" alt="shotify demo workflow">
</p>

---

## What is shotify?

shotify is a native desktop app for capturing, styling, and annotating screenshots without passing images through multiple photo editors. Everything runs locally on your machine with no accounts, trackers, or cloud uploads.

You take a screenshot or paste an existing image from your clipboard, pick a background and frame style, mark up any details, and export directly as PNG, JPEG, or WebP.

<p align="center">
  <img src="./docs/media/interfacescreenshot.png" width="620" alt="shotify main canvas editor">
</p>

---

## Downloads

Precompiled binaries for each platform are available on the [Releases page](https://github.com/Lalitmukesh69/shotify/releases/latest).

### Linux

Packages are built for x86_64:

- **AppImage:** [shotify_linux_amd64.AppImage](https://github.com/Lalitmukesh69/shotify/releases/download/v1.0.0/shotify_linux_amd64.AppImage)
- **Debian / Ubuntu (.deb):** [shotify_linux_amd64.deb](https://github.com/Lalitmukesh69/shotify/releases/download/v1.0.0/shotify_linux_amd64.deb)
- **Fedora / RHEL (.rpm):** [shotify_linux_x86_64.rpm](https://github.com/Lalitmukesh69/shotify/releases/download/v1.0.0/shotify_linux_x86_64.rpm)

```bash
# Run the AppImage
chmod +x shotify_linux_amd64.AppImage
./shotify_linux_amd64.AppImage

# Or install with dpkg
sudo dpkg -i shotify_linux_amd64.deb

# Or install with rpm
sudo rpm -i shotify_linux_x86_64.rpm
```

Wayland users will need `grim`, `slurp`, and `wl-clipboard` installed for full capture and clipboard support:
```bash
# Debian / Ubuntu
sudo apt install grim slurp wl-clipboard

# Arch Linux
sudo pacman -S grim slurp wl-clipboard

# Fedora
sudo dnf install grim slurp wl-clipboard
```

### macOS

- **Apple Silicon (M1/M2/M3/M4):** [shotify_darwin_aarch64.dmg](https://github.com/Lalitmukesh69/shotify/releases/download/v1.0.0/shotify_darwin_aarch64.dmg)
- **Intel:** [shotify_darwin_x64.dmg](https://github.com/Lalitmukesh69/shotify/releases/download/v1.0.0/shotify_darwin_x64.dmg)

Open the `.dmg` file and drag shotify to Applications.

If macOS displays a warning about an unverified developer on first launch, right-click `shotify.app` in Finder and select **Open**, or clear the quarantine flag in your terminal:
```bash
xattr -rd com.apple.quarantine /Applications/shotify.app
```

### Windows

- **Installer (.exe):** [shotify_windows_x64.exe](https://github.com/Lalitmukesh69/shotify/releases/download/v1.0.0/shotify_windows_x64.exe)
- **MSI Package (.msi):** [shotify_windows_x64.msi](https://github.com/Lalitmukesh69/shotify/releases/download/v1.0.0/shotify_windows_x64.msi)

Run the installer and follow the prompt.

---

## Core Capabilities

### Canvas Framing
- Background options include system desktop wallpapers, 30+ preset gradients, soft animated blobs, solid colors, and custom image uploads.
- Window headers replicate macOS traffic lights or Windows titlebars, with an option for borderless frames.
- Canvas controls let you adjust padding (16px to 96px), corner radius (0px to 32px), shadow diffusion, and 3D perspective tilt (-6 deg to +6 deg).
- Preset aspect ratios include 16:9, 4:3, 1:1, Twitter header, or tight auto-crop to the image boundary.

### Annotations
Draw freehand with a pen tool, draw geometric shapes (rectangles, ellipses, arrows), drop numbered step badges (1, 2, 3...) for instructional walkthroughs, and place customizable watermark text.

<p align="center">
  <img src="./docs/media/annotationexample.png" width="620" alt="Annotation tools in shotify">
</p>

### Capture & Import
- Global hotkeys trigger interactive region selection, active window capture, or full screen captures.
- Paste images directly from your clipboard (`Ctrl+V` or `Cmd+V`).
- Drag image files from your file manager directly onto the app window.

### Export
- Copy the finished graphic straight to your clipboard with `Ctrl+C` or `Cmd+C`.
- Save via your system file dialog or configure sequential autosave to a chosen folder.
- Export to PNG, JPEG, or WebP with 1x, 2x, or 3x pixel density scaling.

<p align="center">
  <img src="./docs/media/shotexample.png" width="620" alt="Example styled screenshot export">
</p>

---

## Linux Architecture

shotify is built to work reliably across modern Linux desktop setups:

- **Wayland Compositors:** Tested on Hyprland, Sway, Niri, GNOME Wayland, and KDE Plasma 6.
- **Capture Backends:** Uses `grim` + `slurp` for interactive region selection when available. Falls back to XDG Desktop Portal via `ashpd` on GNOME/KDE, with X11 fallback when running under an X session.
- **Clipboard Management:** Wayland clipboard operations run through `wl-copy` to preserve image MIME types across Wayland client boundaries, falling back to Tauri's internal clipboard under X11.

---

## Building from Source

### 1. Prerequisites

- **Node.js:** 18 or higher with `npm`
- **Rust:** Stable toolchain via [rustup.rs](https://rustup.rs/)

System libraries required by Tauri on Linux:

```bash
# Debian / Ubuntu
sudo apt update && sudo apt install -y \
  build-essential curl wget file libssl-dev \
  libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  grim slurp wl-clipboard

# Fedora
sudo dnf install -y \
  webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel \
  grim slurp wl-clipboard
```

On macOS, install the Xcode Command Line Tools (`xcode-select --install`).  
On Windows, install the C++ build tools from Microsoft Visual Studio and the WebView2 runtime.

### 2. Development

```bash
# Clone the repository
git clone https://github.com/Lalitmukesh69/shotify.git
cd shotify

# Install dependencies
npm install

# Start Vite and the Tauri application in dev mode
npm run dev

# Run unit and integration tests
npm test
```

### 3. Production Build

```bash
npm run tauri:build
```

Compiled bundles will be generated under `src-tauri/target/release/bundle/`.

---

## Project Layout

```text
shotify/
├── components/          # React dialogs and modal components
├── docs/media/          # Screenshots, diagrams, and demo assets
├── landing/             # Static product website hosted on GitHub Pages
├── platform/            # Platform setup scripts (Linux Wayland, macOS, Windows)
├── src-tauri/           # Rust backend (Tauri v2 core, shortcuts, capture logic)
├── tests/               # Vitest unit test suite (72+ tests)
├── ui/                  # React canvas frontend (Vite)
└── .github/workflows/   # CI tests, GitHub Pages deploy, and release builds
```

---

## Landing Page

The product landing page lives in the [`landing/`](./landing/) folder and is deployed automatically to GitHub Pages:

**[https://lalitmukesh69.github.io/shotify/](https://lalitmukesh69.github.io/shotify/)**

To test it locally:
```bash
python3 -m http.server 4173 --directory landing
```
Open `http://localhost:4173` in your browser.

---

## License

shotify is released under the **GNU General Public License v3.0 (GPL-3.0)**. See [LICENSE](./LICENSE) for the full text.

Portions derived from upstream [shotify](https://github.com/webadderall/shot) are Copyright (c) 2026 shotify and retain their original notice in [NOTICE](./NOTICE).
