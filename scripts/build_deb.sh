#!/usr/bin/env bash
set -euo pipefail

VERSION="0.1.5"
echo "==> Packaging openre_amd64.deb v${VERSION}"

BUILD_ROOT="build/deb"
rm -rf "${BUILD_ROOT}"
mkdir -p "${BUILD_ROOT}/DEBIAN"
mkdir -p "${BUILD_ROOT}/usr/bin"
mkdir -p "${BUILD_ROOT}/usr/share/applications"
mkdir -p "${BUILD_ROOT}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${BUILD_ROOT}/usr/share/doc/openre"

# Copy binaries
if [ -f "target/release/OpenRe" ]; then
    cp "target/release/OpenRe" "${BUILD_ROOT}/usr/bin/openre"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre"
elif [ -f "target/release/openre-cli" ]; then
    cp "target/release/openre-cli" "${BUILD_ROOT}/usr/bin/openre"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre"
else
    echo "Executable not found! Run cargo build --release first."
    exit 1
fi

if [ -f "target/release/openre-cli" ]; then
    cp "target/release/openre-cli" "${BUILD_ROOT}/usr/bin/openre-cli"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre-cli"
fi

# Create icon
cat << 'EOF' > "${BUILD_ROOT}/usr/share/icons/hicolor/scalable/apps/openre.svg"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <rect width="100" height="100" rx="20" fill="#0284c7"/>
  <text x="50" y="65" font-family="system-ui, sans-serif" font-size="44" font-weight="bold" fill="#ffffff" text-anchor="middle">Re</text>
</svg>
EOF

# Create desktop entry
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

# Copy documentation
cp README.md "${BUILD_ROOT}/usr/share/doc/openre/"
cp LICENSE "${BUILD_ROOT}/usr/share/doc/openre/copyright"
if [ -f "docs/installation/INSTALLATION_PT_BR.md" ]; then
    cp docs/installation/INSTALLATION_PT_BR.md "${BUILD_ROOT}/usr/share/doc/openre/"
fi
if [ -f "docs/installation/INSTALLATION_EN.md" ]; then
    cp docs/installation/INSTALLATION_EN.md "${BUILD_ROOT}/usr/share/doc/openre/"
fi

# Create Debian control file
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

# Build package
dpkg-deb --build "${BUILD_ROOT}" openre_amd64.deb
echo "Successfully created openre_amd64.deb v${VERSION}"
