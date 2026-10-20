#!/usr/bin/env bash
set -euo pipefail

echo "==> Packaging openre_amd64.deb"

BUILD_ROOT="build/deb"
rm -rf "${BUILD_ROOT}"
mkdir -p "${BUILD_ROOT}/DEBIAN"
mkdir -p "${BUILD_ROOT}/usr/bin"
mkdir -p "${BUILD_ROOT}/usr/share/applications"
mkdir -p "${BUILD_ROOT}/usr/share/icons/hicolor/512x512/apps"
mkdir -p "${BUILD_ROOT}/usr/share/doc/openre"

# Copy binary
if [ -f "target/release/openre-cli" ]; then
    cp "target/release/openre-cli" "${BUILD_ROOT}/usr/bin/openre"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre"
elif [ -f "target/release/openre" ]; then
    cp "target/release/openre" "${BUILD_ROOT}/usr/bin/openre"
    chmod 755 "${BUILD_ROOT}/usr/bin/openre"
else
    echo "Executable not found! Run cargo build --release first."
    exit 1
fi

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
EOF
chmod 644 "${BUILD_ROOT}/usr/share/applications/openre.desktop"

# Copy documentation
cp README.md "${BUILD_ROOT}/usr/share/doc/openre/"
cp LICENSE "${BUILD_ROOT}/usr/share/doc/openre/copyright"

# Create Debian control file
cat << 'EOF' > "${BUILD_ROOT}/DEBIAN/control"
Package: openre
Version: 0.1.0
Section: utils
Priority: optional
Architecture: amd64
Maintainer: OpenRe Maintainers <maintainers@openre.org>
Description: Open-Source, Cross-Platform File Recovery Software
 OpenRe is a genuine, offline-first digital forensics and deleted file recovery
 application. It features NTFS/FAT32/exFAT/ext4 metadata recovery, deep block
 carving for images, documents, and media, pre-recovery preview, and safe
 non-destructive file repair.
EOF

# Build package
dpkg-deb --build "${BUILD_ROOT}" openre_amd64.deb
echo "Successfully created openre_amd64.deb"
