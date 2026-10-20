# Building OpenRe from Source

**Language & Requirements:**  
- Rust 1.80+ (Stable)
- Node.js 20+ (LTS) & npm 10+
- Cargo & Git

---

## 1. Quick Build (CLI & Recovery Core)

To compile the entire Rust recovery workspace and the `openre-cli` diagnostic tool:

```bash
# Clone the repository
git clone https://github.com/openre/openre.git
cd openre

# Build all workspace crates in release mode
cargo build --release

# Run the test suite
cargo test
```

The compiled binaries will be placed in `target/release/`:
- `openre-cli.exe` (on Windows) or `openre-cli` (on Linux)

---

## 2. Desktop Application Build (Tauri v2 GUI)

### 2.1 Install Frontend Dependencies
```bash
# In the project root:
npm install
```

### 2.2 Run in Development Mode
```bash
# Starts the frontend Vite dev server and launches the Tauri window with hot-reload
npm run tauri dev
```

### 2.3 Build Production Distribution
```bash
# Builds the frontend bundle and compiles the native executable
npm run tauri build
```

The output executable is generated at:
- `target/release/OpenRe.exe` (Windows)
- `target/release/bundle/deb/openre_amd64.deb` (Ubuntu Linux)

---

## 3. Platform Specific Notes

### Windows (MSVC or GNU)
- When compiling with MSVC: Ensure Visual Studio C++ Build Tools and Windows SDK (10.0.19041+) are installed.
- When compiling with GNU: Use the `x86_64-pc-windows-gnu` target with MinGW-w64 (such as WinLibs GCC 14+ or LLVM-MinGW).

### Ubuntu / Debian Linux
Install native build dependencies:
```bash
sudo apt update
sudo apt install -y build-essential curl wget file libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```
