# Release Engineering & Packaging — OpenRe

**Target Artifacts:**
- Windows Portable: `OpenRe-Windows-x64-Portable.zip`
- Ubuntu Debian Package: `openre_amd64.deb`
- Linux AppImage: `OpenRe-Linux-x64.AppImage`
- Cryptographic Integrity: `SHA256SUMS.txt`

---

## 1. Release Process Overview

1. **Pre-Release Audit:**
   - Run `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
   - Run full synthetic test suite `cargo test`.
   - Run git hygiene audit to ensure no secrets, credentials, or personal files are tracked.
2. **Version Bumping:**
   - Update `version` in workspace `Cargo.toml`, crate `Cargo.toml` files, `package.json`, and `src-tauri/tauri.conf.json`.
   - Update `CHANGELOG.md` with release highlights.
3. **Automated CI Build:**
   - Push a semantic version tag (e.g. `v0.1.0`).
   - GitHub Actions workflow `.github/workflows/release.yml` triggers on the tag.
   - Builds Windows Portable ZIP on `windows-latest`.
   - Builds Ubuntu `.deb` on `ubuntu-22.04`.
   - Generates SHA-256 checksums for all release binaries.
   - Creates an official GitHub Release with release notes and checksums.

---

## 2. Windows Portable ZIP Construction

The portable Windows package structure:
```text
OpenRe-Windows-x64-Portable/
├── OpenRe.exe             # Native application executable
├── README.txt             # Quick instructions & safety warning
├── LICENSE                # Open-source license terms
└── docs/                  # Offline documentation & Portuguese guide
```

The user simply extracts the ZIP and runs `OpenRe.exe`. No installation, registry modifications, or development tools are required.
