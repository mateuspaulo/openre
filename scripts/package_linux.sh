#!/usr/bin/env bash
set -euo pipefail

# Extract version from Cargo.toml or default to 0.1.5
VERSION=$(grep -m1 '^version' Cargo.toml | awk -F '"' '{print $2}')
if [ -z "$VERSION" ]; then
    VERSION="0.1.5"
fi

echo "==> Packaging OpenRe v${VERSION} for Linux (x64)"

RELEASE_DIR="release"
mkdir -p "${RELEASE_DIR}"

# -------------------------------------------------------------
# 1. Debian Package (openre_amd64.deb)
# -------------------------------------------------------------
echo "==> Building Debian Package (openre_amd64.deb)..."
BUILD_ROOT="build/deb"
rm -rf "${BUILD_ROOT}"
mkdir -p "${BUILD_ROOT}/DEBIAN"
mkdir -p "${BUILD_ROOT}/usr/bin"
mkdir -p "${BUILD_ROOT}/usr/share/applications"
mkdir -p "${BUILD_ROOT}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${BUILD_ROOT}/usr/share/doc/openre"

# Determine GUI vs CLI binary paths
BIN_GUI=""
BIN_CLI=""

if [ -f "target/release/OpenRe" ]; then
    BIN_GUI="target/release/OpenRe"
elif [ -f "target/x86_64-unknown-linux-gnu/release/OpenRe" ]; then
    BIN_GUI="target/x86_64-unknown-linux-gnu/release/OpenRe"
fi

if [ -f "target/release/openre-cli" ]; then
    BIN_CLI="target/release/openre-cli"
elif [ -f "target/x86_64-unknown-linux-gnu/release/openre-cli" ]; then
    BIN_CLI="target/x86_64-unknown-linux-gnu/release/openre-cli"
fi

if [ -n "$BIN_GUI" ]; then
    cp "$BIN_GUI" "${BUILD_ROOT}/usr/bin/openre"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre"
elif [ -n "$BIN_CLI" ]; then
    echo "Warning: OpenRe GUI binary not found, using openre-cli as fallback."
    cp "$BIN_CLI" "${BUILD_ROOT}/usr/bin/openre"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre"
else
    echo "Error: No executable found! Please run cargo build --release first."
    exit 1
fi

if [ -n "$BIN_CLI" ]; then
    cp "$BIN_CLI" "${BUILD_ROOT}/usr/bin/openre-cli"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre-cli"
fi

# Application Icon
cat << 'EOF' > "${BUILD_ROOT}/usr/share/icons/hicolor/scalable/apps/openre.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <rect width="100" height="100" rx="20" fill="#0284c7"/>
  <text x="50" y="65" font-family="system-ui, sans-serif" font-size="44" font-weight="bold" fill="#ffffff" text-anchor="middle">Re</text>
</svg>
EOF

# Desktop Entry
cat << 'EOF' > "${BUILD_ROOT}/usr/share/applications/openre.desktop"
[Desktop Entry]
Name=OpenRe
Comment=Open-Source, Privacy-First File Recovery Software
Exec=/usr/bin/openre
Icon=openre
Terminal=false
Type=Application
Categories=System;Utility;Filesystem;
Keywords=recovery;undelete;carving;forensics;disk;
EOF
chmod 644 "${BUILD_ROOT}/usr/share/applications/openre.desktop"

# Documentation
cp README.md "${BUILD_ROOT}/usr/share/doc/openre/"
cp LICENSE "${BUILD_ROOT}/usr/share/doc/openre/copyright"
if [ -f "docs/installation/INSTALLATION_PT_BR.md" ]; then
    cp docs/installation/INSTALLATION_PT_BR.md "${BUILD_ROOT}/usr/share/doc/openre/"
fi
if [ -f "docs/installation/INSTALLATION_EN.md" ]; then
    cp docs/installation/INSTALLATION_EN.md "${BUILD_ROOT}/usr/share/doc/openre/"
fi

# Debian Control File
cat << EOF > "${BUILD_ROOT}/DEBIAN/control"
Package: openre
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: OpenRe Maintainers <maintainers@openre.org>
Depends: libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37, libgtk-3-0
Description: Open-Source, Cross-Platform File Recovery Software
 OpenRe is a genuine, offline-first digital forensics and deleted file recovery
 application. It features NTFS/FAT32/exFAT/ext4 metadata recovery, deep block
 carving for images, documents, and media, pre-recovery preview, and safe
 non-destructive file repair.
EOF

dpkg-deb --root-owner-group --build "${BUILD_ROOT}" "${RELEASE_DIR}/openre_amd64.deb"
cd "${RELEASE_DIR}"
sha256sum openre_amd64.deb > openre_amd64.deb.sha256
cd ..
echo "==> Debian package generated at ${RELEASE_DIR}/openre_amd64.deb"

# -------------------------------------------------------------
# 2. Universal Portable Tarball (OpenRe-Linux-x64.tar.gz)
# -------------------------------------------------------------
echo "==> Building Universal Portable Tarball (OpenRe-Linux-x64.tar.gz)..."
TAR_ROOT="build/tar"
PKG_DIR="${TAR_ROOT}/OpenRe-Linux-x64"
rm -rf "${TAR_ROOT}"
mkdir -p "${PKG_DIR}/docs"

if [ -n "$BIN_GUI" ]; then
    cp "$BIN_GUI" "${PKG_DIR}/openre"
    chmod 755 "${PKG_DIR}/openre"
fi

if [ -n "$BIN_CLI" ]; then
    cp "$BIN_CLI" "${PKG_DIR}/openre-cli"
    chmod 755 "${PKG_DIR}/openre-cli"
    if [ -z "$BIN_GUI" ]; then
        cp "$BIN_CLI" "${PKG_DIR}/openre"
        chmod 755 "${PKG_DIR}/openre"
    fi
fi

cp README.md "${PKG_DIR}/README.txt"
cp LICENSE "${PKG_DIR}/LICENSE"
if [ -f "docs/installation/INSTALLATION_EN.md" ]; then
    cp docs/installation/INSTALLATION_EN.md "${PKG_DIR}/docs/"
fi
if [ -f "docs/installation/INSTALLATION_PT_BR.md" ]; then
    cp docs/installation/INSTALLATION_PT_BR.md "${PKG_DIR}/docs/"
fi

tar -czvf "${RELEASE_DIR}/OpenRe-Linux-x64.tar.gz" -C "${TAR_ROOT}" OpenRe-Linux-x64
cd "${RELEASE_DIR}"
sha256sum OpenRe-Linux-x64.tar.gz > OpenRe-Linux-x64.tar.gz.sha256
cd ..
echo "==> Tarball generated at ${RELEASE_DIR}/OpenRe-Linux-x64.tar.gz"

echo "==> Linux packaging complete!"
