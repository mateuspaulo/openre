# OpenRe — Installation & User Guide (English)

Welcome to **OpenRe**, a high-performance, open-source, privacy-first file recovery application for Windows and Ubuntu Linux.

---

## 1. System Requirements

### Windows
- **Operating System:** Windows 10 (64-bit) or Windows 11 (64-bit).
- **Runtime:** Microsoft Edge WebView2 (pre-installed on Windows 11 and modern Windows 10 updates).
- **Dependencies:** None. The portable release is 100% self-contained.

### Ubuntu Linux
- **Operating System:** Ubuntu 22.04 LTS, Ubuntu 24.04 LTS, or compatible Debian-based distribution.
- **Dependencies:** `libwebkit2gtk-4.1` (or `libwebkit2gtk-4.0`), `libayatana-appindicator3-1`.

---

## 2. Installation Instructions

### 2.1 Windows (Portable Release)
1. Download `OpenRe-Windows-x64-Portable.zip` from the official GitHub Releases page.
2. Extract the ZIP archive to your preferred location (e.g. `C:\Tools\OpenRe` or a portable USB drive).
   - **Crucial Rule:** Do NOT extract or run OpenRe on the exact same drive partition you are trying to recover data from!
3. Double-click `OpenRe.exe` to launch the application.

### 2.2 Ubuntu Linux (.deb Package)
1. Download `openre_amd64.deb` from the official GitHub Releases page.
2. Open a terminal and install the package using `apt`:
   ```bash
   sudo apt update
   sudo apt install ./openre_amd64.deb
   ```
3. Launch OpenRe from your application menu or run:
   ```bash
   openre
   ```

---

## 3. Permissions & Administrator Elevation

- **Scanning Folders and Disk Images:** Does NOT require administrator privileges. You can inspect folder trees, raw disk images (`.img`, `.dd`), and virtual disks in normal user mode.
- **Scanning Physical Drives and Raw Volumes:** Windows requires Administrator privileges (`Run as administrator`) and Linux requires `sudo` or disk group permissions to read physical sector devices (`\\.\PhysicalDrive0`, `/dev/sdb`).
- **Safe Elevation:** OpenRe never silently escalates permissions. When direct sector access is requested, the system elevation prompt (UAC on Windows, Polkit on Linux) will appear.

---

## 4. Best Practices for Safe Recovery

1. **Stop using the affected drive immediately:** Avoid writing new files, browsing heavily, or downloading updates on the damaged media.
2. **Always choose a different destination device:** Never recover files onto the same drive being scanned. Save recovered files to an external hard drive, USB flash drive, or separate internal disk.
3. **Use Disk Images for failing drives:** If a hard drive is making clicking noises or has bad sectors, create a raw disk image first using OpenRe's Disk Imaging tool, and perform all recovery scans against the image file.
