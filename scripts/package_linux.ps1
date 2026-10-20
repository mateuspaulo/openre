$ErrorActionPreference = "Stop"

$root = "C:\Users\Mateus\Desktop\OpenRe"
$zigExe = "C:\Users\Mateus\AppData\Local\Microsoft\WinGet\Packages\zig.zig_Microsoft.Winget.Source_8wekyb3d8bbwe\zig-x86_64-windows-0.17.0\zig.exe"
$binSource = "$root\target\x86_64-unknown-linux-musl\release\openre-cli"
$releaseDir = "$root\release"

if (-not (Test-Path $binSource)) {
    Write-Error "Linux binary not found at $binSource"
}

Write-Host "==> Packaging 1: Universal Portable Tarball (OpenRe-Linux-x64.tar.gz)"
$tarWork = "$releaseDir\linux_tar_work"
$pkgDir = "$tarWork\OpenRe-Linux-x64"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tarWork
New-Item -ItemType Directory -Path "$pkgDir\docs" -Force | Out-Null

Copy-Item $binSource "$pkgDir\openre"
Copy-Item $binSource "$pkgDir\openre-cli"
Copy-Item "$root\README.md" "$pkgDir\README.txt"
Copy-Item "$root\LICENSE" "$pkgDir\LICENSE"
Copy-Item "$root\docs\installation\INSTALLATION_EN.md" "$pkgDir\docs\"
Copy-Item "$root\docs\installation\INSTALLATION_PT_BR.md" "$pkgDir\docs\"

$tarTarget = "$releaseDir\OpenRe-Linux-x64.tar.gz"
Remove-Item -Force -ErrorAction SilentlyContinue $tarTarget
tar -czvf $tarTarget -C $tarWork OpenRe-Linux-x64
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tarWork

Write-Host "==> Packaging 2: Debian Package (openre_amd64.deb)"
$debWork = "$releaseDir\deb_work"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $debWork
New-Item -ItemType Directory -Path "$debWork\control_dir" -Force | Out-Null
New-Item -ItemType Directory -Path "$debWork\data_dir\usr\bin" -Force | Out-Null
New-Item -ItemType Directory -Path "$debWork\data_dir\usr\share\applications" -Force | Out-Null
New-Item -ItemType Directory -Path "$debWork\data_dir\usr\share\icons\hicolor\scalable\apps" -Force | Out-Null
New-Item -ItemType Directory -Path "$debWork\data_dir\usr\share\doc\openre" -Force | Out-Null

# Binaries
Copy-Item $binSource "$debWork\data_dir\usr\bin\openre"
Copy-Item $binSource "$debWork\data_dir\usr\bin\openre-cli"

# Docs
Copy-Item "$root\README.md" "$debWork\data_dir\usr\share\doc\openre\README.md"
Copy-Item "$root\LICENSE" "$debWork\data_dir\usr\share\doc\openre\copyright"
Copy-Item "$root\docs\installation\INSTALLATION_PT_BR.md" "$debWork\data_dir\usr\share\doc\openre\"

# Desktop Entry
$desktop = @"
[Desktop Entry]
Name=OpenRe
Comment=Open-Source, Privacy-First File Recovery Software
Exec=/usr/bin/openre
Icon=openre
Terminal=true
Type=Application
Categories=System;Utility;Filesystem;
Keywords=recovery;undelete;carving;forensics;disk;
"@
[System.IO.File]::WriteAllText("$debWork\data_dir\usr\share\applications\openre.desktop", $desktop, [System.Text.Encoding]::UTF8)

# Icon
$svg = @"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <rect width="100" height="100" rx="20" fill="#0284c7"/>
  <text x="50" y="65" font-family="system-ui, sans-serif" font-size="44" font-weight="bold" fill="#ffffff" text-anchor="middle">Re</text>
</svg>
"@
[System.IO.File]::WriteAllText("$debWork\data_dir\usr\share\icons\hicolor\scalable\apps\openre.svg", $svg, [System.Text.Encoding]::UTF8)

# Control File
$control = @"
Package: openre
Version: 0.1.5
Section: utils
Priority: optional
Architecture: amd64
Maintainer: OpenRe Maintainers <maintainers@openre.org>
Description: Open-Source, Privacy-First File Recovery Software
 OpenRe is a genuine, offline-first digital forensics and deleted file recovery
 application. Features NTFS/FAT32/exFAT/ext4 metadata recovery, deep block
 carving for images, documents, and media, pre-recovery preview, and safe
 non-destructive file repair.
"@
# Note: Debian control files require a trailing newline and LF endings
$controlLf = $control.Replace("`r`n", "`n") + "`n"
[System.IO.File]::WriteAllText("$debWork\control_dir\control", $controlLf, [System.Text.Encoding]::ASCII)

# Debian Binary spec
$debBinary = "2.0`n"
[System.IO.File]::WriteAllText("$debWork\debian-binary", $debBinary, [System.Text.Encoding]::ASCII)

# Create control.tar.gz and data.tar.gz
tar -czvf "$debWork\control.tar.gz" -C "$debWork\control_dir" control
tar -czvf "$debWork\data.tar.gz" -C "$debWork\data_dir" usr

# Assemble final .deb archive with zig ar
$debTarget = "$releaseDir\openre_amd64.deb"
Remove-Item -Force -ErrorAction SilentlyContinue $debTarget

Set-Location $debWork
& $zigExe ar rcs "$debTarget" debian-binary control.tar.gz data.tar.gz
Set-Location $root

Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $debWork

Write-Host "==> Verifying created release packages:"
Get-Item "$releaseDir\openre_amd64.deb", "$releaseDir\OpenRe-Linux-x64.tar.gz"
