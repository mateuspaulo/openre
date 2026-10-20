# Changelog

All notable changes to **OpenRe** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-10-20

### Added
- **Results Pagination Engine (Step 5):** Configurable pagination with page sizes of `20`, `50`, `100`, `200`, and `300` items per page. Greatly optimizes DOM rendering performance and memory usage, eliminating UI freezes when reviewing thousands of carved files.
- **Dual-Level Selection Toolbar:** Quick-selection controls enabling users to select or deselect either the current page or all filtered items at once.
- **Real Multimedia Players in Preview Drawer:**
  - **Audio Player:** Native HTML5 audio player (`<audio controls>`) capable of playing recovered MP3, WAV, OGG, FLAC, and M4A streams directly from carved sectors.
  - **Video Player:** Native HTML5 video player (`<video controls>`) for playback of recovered MP4, WebM, and other video streams.
  - **Image Viewer:** Full-resolution image display with dimensions indicator and format badge.
  - **Decoded Text Reader:** Monospace viewer for textual files and snippets.
- **Preview Drawer Mode Switcher:** Tabbed interface distinguishing between "Visual / Media Preview" (default) and "Bytecode Hex Dump" for low-level forensic sector examination.
- **Expanded Preview Buffer:** Scaled backend preview buffer up to 12 MB in `get_file_preview` to feed complete media headers and streams to the frontend without sector cutoff.

## [0.1.1] - 2026-10-20

### Added
- **Docked Footer Diagnostic Terminal:** Live logging console embedded into the application footer showing real-time disk reads, block scanning progress, carves, warnings, and error messages.
- **Terminal Controls:** Enabled by default with immediate close ("X") button, Settings modal toggle switch, top header shortcut button, auto-scroll, and one-click copy to clipboard.
- **Folder Virtual Block Reader:** Resilient recursive directory stream reader (`FolderBlockReader`) capable of deep carving across any user folder without requiring administrator elevation.
- **Sector-Aligned Windows Volume Reader:** Direct physical partition reader (`VolumeBlockReader`) using 4096-byte aligned I/O for Windows raw volumes (`\\.\C:`, `\\.\D:`).

### Fixed
- **Premature Deep Scan Termination:** Resolved issue where locked system files (e.g. `DumpStack.log.tmp`) caused `FolderBlockReader` to return zero bytes, prematurely breaking out of the carving loop.
- **Un-Elevated Drive Access:** Added automatic graceful fallback when raw volume access is denied by Windows, streaming files across the drive root without freezing.
- **Scan Progress Throttling:** Capped UI IPC event dispatch to 100 ms (10 FPS) to prevent WebView2 event queue saturation during high-throughput (250+ MB/s) carving.

## [0.1.0] - 2026-10-20

### Added
- **Core Recovery Engine:** High-performance Rust-based recovery workspace across 8 dedicated crates.
- **Filesystem Parsers:**
  - NTFS: Master File Table (`$MFT`) record decoder, resident/non-resident `$DATA` runlists, `$STANDARD_INFORMATION` timestamps, `$FILE_NAME` parent references.
  - FAT32: Boot Parameter Block (BPB) parser, directory table traversal, Long File Name (LFN) reassembly, deleted `0xE5` entry undeletion.
  - exFAT: Volume parameter calculation, cluster allocation bitmap, directory entries (`0x85` file, `0xC0` stream, `0xC1` name).
  - ext4: Superblock validation, inode table reading, extent tree decoding, deleted inode recognition.
- **Format-Aware Deep Carving:**
  - Images: JPEG/JPG, PNG, GIF, WebP, BMP, TIFF with chunk and marker verification.
  - Documents: PDF, TXT, DOCX, XLSX, PPTX, ZIP with central directory validation.
  - Media: MP3, WAV, MP4, MOV, MKV container and atom traversal.
- **Pre-Recovery Sandbox Preview:** Safe in-app thumbnail generation, hex dump viewer with ASCII column, and document metadata extraction.
- **Non-Destructive File Repair:** Automated header reconstruction and missing marker fixups for JPEG/PNG on derivative copies without modifying original recovered files.
- **Disk Imaging Tool:** Byte-for-byte `.img` / `.dd` creation with bad sector skipping and unreadable region logging.
- **Desktop Graphical Interface:** Modern Tauri v2 interface with 7-step guided recovery workflow, dark/light theme, search, filtering, and destination collision prevention.
- **Security & Privacy:** Enforced offline-only architecture, zero telemetry, automated network isolation test suite.
- **Documentation:** Complete English technical guides and Brazilian Portuguese installation manual (`INSTALLATION_PT_BR.md`).
