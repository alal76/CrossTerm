---
title: "Installation & Upgrade"
slug: "installation"
category: "basics"
order: 0
schema_version: 1
keywords: ["install", "download", "homebrew", "brew", "deb", "rpm", "appimage", "msi", "exe", "upgrade", "uninstall", "windows", "macos", "linux"]
---

# Installation & Upgrade

CrossTerm ships as a native binary for macOS, Windows, and Linux. No runtime dependencies are required — the installer bundles everything.

---

## macOS

### Homebrew (recommended)

Homebrew manages installation, upgrades, and uninstall automatically.

```bash
brew tap alal76/crossterm
brew install --cask crossterm
```

CrossTerm.app is placed in `/Applications`.

**Upgrade:**
```bash
brew upgrade --cask crossterm
```

**Check for updates:**
```bash
brew outdated --cask
```

**Uninstall:**
```bash
brew uninstall --cask crossterm
brew untap alal76/crossterm   # optional: remove the tap
```

### Direct DMG

1. Download `CrossTerm_x.y.z_aarch64.dmg` from the [releases page](https://github.com/alal76/CrossTerm/releases/latest).
2. Open the downloaded `.dmg` file.
3. Drag **CrossTerm.app** into your **Applications** folder.
4. Eject the disk image.

On first launch macOS may show a Gatekeeper dialog — open **System Settings → Privacy & Security** and click **Open Anyway** if prompted.

**Upgrade:** download and install the new DMG; it replaces the existing app.

**Uninstall:** drag `CrossTerm.app` from `/Applications` to the Trash. To remove user data:
```bash
rm -rf ~/Library/Application\ Support/com.crossterm.app
rm -rf ~/Library/Preferences/com.crossterm.app.plist
rm -rf ~/Library/Caches/com.crossterm.app
```

### Verify checksum (optional)

Every release includes a `.sha256` file. To verify:
```bash
shasum -a 256 -c CrossTerm_0.2.5_aarch64.dmg.sha256
```

---

## Windows

### Installer (.exe)

1. Download `CrossTerm_x.y.z_x64-setup.exe` from the [releases page](https://github.com/alal76/CrossTerm/releases/latest).
2. Run the installer and follow the wizard.
3. CrossTerm is added to the Start menu and optionally to the Desktop.

**Upgrade:** run the new installer — it detects and replaces the existing version.

**Uninstall:** Settings → Apps → CrossTerm → Uninstall, or use the bundled uninstaller in the install directory.

### MSI package (enterprise/GPO)

The `.msi` package supports silent/unattended installation via Group Policy or SCCM.

**Silent install:**
```powershell
msiexec /i CrossTerm_0.2.5_x64_en-US.msi /quiet /norestart
```

**Silent uninstall:**
```powershell
msiexec /x CrossTerm_0.2.5_x64_en-US.msi /quiet /norestart
```

### Verify checksum (optional)

```powershell
Get-FileHash CrossTerm_0.2.5_x64-setup.exe -Algorithm SHA256
# Compare output to CrossTerm_0.2.5_x64-setup.exe.sha256
```

---

## Linux

### Debian / Ubuntu — .deb package

```bash
# Download
wget https://github.com/alal76/CrossTerm/releases/latest/download/CrossTerm_0.2.5_amd64.deb

# Install
sudo dpkg -i CrossTerm_0.2.5_amd64.deb

# Fix any missing dependencies
sudo apt-get install -f
```

CrossTerm is added to the application launcher and available as `crossterm` in PATH.

**Upgrade:**
```bash
# dpkg -i handles upgrade automatically when a newer version is installed
sudo dpkg -i CrossTerm_new_version_amd64.deb
```

**Uninstall:**
```bash
sudo dpkg -r crossterm
```

### Red Hat / Fedora / SUSE — .rpm package

```bash
# Fedora / RHEL
sudo rpm -i CrossTerm-0.2.5-1.x86_64.rpm

# Or with dnf (handles dependencies)
sudo dnf install CrossTerm-0.2.5-1.x86_64.rpm
```

**Upgrade:**
```bash
sudo rpm -U CrossTerm-new_version-1.x86_64.rpm
```

**Uninstall:**
```bash
sudo rpm -e crossterm-app
```

### AppImage — universal portable binary

AppImages run on any modern Linux distribution without installation.

```bash
# Download
wget https://github.com/alal76/CrossTerm/releases/latest/download/CrossTerm_0.2.5_amd64.AppImage

# Make executable
chmod +x CrossTerm_0.2.5_amd64.AppImage

# Run directly
./CrossTerm_0.2.5_amd64.AppImage
```

To integrate with your desktop launcher, use [AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher) or move the AppImage to `~/.local/bin/`.

**Upgrade:** download the new AppImage, make it executable, and replace the old file.

### Verify checksum

```bash
sha256sum -c CrossTerm_0.2.5_amd64.AppImage.sha256
```

---

## System Requirements

| Platform | Minimum |
|----------|---------|
| macOS | 11.0 (Big Sur) or later, Apple Silicon or Intel |
| Windows | Windows 10 (1903) or later, 64-bit |
| Linux | Ubuntu 22.04 / Fedora 37 or equivalent; glibc 2.35+ |
| RAM | 256 MB (512 MB recommended) |
| Disk | 80 MB |

CrossTerm uses the system WebView (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux). WebView2 is pre-installed on Windows 11 and Windows 10 May 2021 Update+; older Windows 10 systems will be prompted to install it automatically.

---

## Release channels

All releases are published on the [GitHub releases page](https://github.com/alal76/CrossTerm/releases). There is currently one channel:

| Channel | Description |
|---------|-------------|
| **Stable** | Tagged `vX.Y.Z` releases — recommended for all users |

Release notes, checksums, and all platform artifacts are attached to each release.
