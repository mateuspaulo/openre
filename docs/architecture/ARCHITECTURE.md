# OpenRe Architecture Specification

**System:** OpenRe File Recovery Application  
**Version:** 0.1.0  
**Language:** Rust (Engine), TypeScript / React (UI), Tauri v2 (Host Application)  

---

## 1. Architectural Overview

OpenRe is designed as a modular, layered recovery system with strict boundaries between low-level hardware/disk operations, forensic parsing algorithms, safe file extraction, and the desktop presentation layer.

```
┌─────────────────────────────────────────────────────────────┐
│                   Desktop User Interface                    │
│      React + TypeScript + Tailwind CSS (Light / Dark)       │
└──────────────────────────────┬──────────────────────────────┘
                               │ Tauri v2 IPC (Events & Commands)
┌──────────────────────────────▼──────────────────────────────┐
│                      Tauri Host App                         │
│                     (`src-tauri/src`)                       │
└──────┬───────────────────────┬───────────────────────┬──────┘
       │                       │                       │
┌──────▼────────┐       ┌──────▼────────┐       ┌──────▼────────┐
│  Device       │       │  Scanner      │       │  File         │
│  Access       │       │  (Quick &     │       │  Preview      │
│  (`device-    │       │   Deep)       │       │  (`file-      │
│   access`)    │       │  (`scanner`)  │       │   preview`)   │
└──────┬────────┘       └──────┬────────┘       └──────┬────────┘
       │                       │                       │
┌──────▼────────┐       ┌──────▼────────┐       ┌──────▼────────┐
│  Filesystem   │       │  File         │       │  Image        │
│  Parsers      │       │  Carving      │       │  Repair       │
│  (`filesystem │       │  (`file-      │       │  (`image-     │
│   `)          │       │   carving`)   │       │   repair`)    │
└──────┬────────┘       └──────┬────────┘       └──────┬────────┘
       │                       │                       │
┌──────┴───────────────────────┴───────────────────────┴──────┐
│                    Recovery Core Engine                     │
│      Types, Session, Integrity, Bounded Buffers, Safety      │
│                     (`recovery-core`)                       │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│                 Reporting & Safe Exporter                   │
│              Audit Reports & Non-Overwriting Output         │
│                     (`reporting`)                           │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Workspace Crates & Responsibilities

### 2.1 `crates/recovery-core`
- **Domain Models:** `RecoveredFile`, `FileCategory`, `IntegrityStatus`, `ScanProgress`, `DeviceInfo`, `PartitionInfo`.
- **Concurrency & Control:** `CancellationToken` for instant, non-blocking scan cancellation.
- **Session Management:** Serializable scan session states for resuming long-running scans.
- **Safety Assertions:** Path sanitization and target drive collision verification.

### 2.2 `crates/device-access`
- **Abstraction:** `BlockReader` trait providing seekable, block-bounded read operations.
- **Implementations:**
  - `PhysicalDriveReader`: Windows native raw disk reading (`\\.\PhysicalDrive0`).
  - `VolumeReader`: Logical drive volume reading (`\\.\C:`, `\\.\D:`).
  - `DiskImageReader`: Universal raw image reading (`.img`, `.dd`, `.raw`).
  - `DirectoryReader`: Safe folder-level scanner when raw disk elevation is unpermitted.
- **Disk Imaging:** Raw sector cloner with bad sector skipping and error map logging.

### 2.3 `crates/filesystem`
- **NTFS:**
  - Boot sector & BIOS Parameter Block (BPB) extraction.
  - `$MFT` record decoding (record header `FILE`, record allocation flags `0x00` deleted vs `0x01` active).
  - Standard Information (`$STANDARD_INFORMATION`) timestamps (creation, alteration, MFT modification, read).
  - File Name (`$FILE_NAME`) attribute parser for unicode filenames and parent directory record numbers.
  - Data (`$DATA`) attribute parsing: Resident byte buffer extraction and Non-Resident cluster runlists decoding.
- **FAT32:**
  - BPB validation, Root Directory cluster locator, FAT table cluster chain traversal.
  - 32-byte Directory Entry parser: Deleted status byte (`0xE5`), short 8.3 names, Long File Name (LFN) sequence reassembly, cluster allocation pointers, and file size.
- **exFAT:**
  - Volume parameters, cluster size calculation.
  - Primary directory entry (`0x85` active / `0x05` deleted), Stream extension (`0xC0`), File Name entries (`0xC1`).
- **ext4:**
  - Superblock magic verification (`0xEF53`), block size, blocks per group.
  - Inode table traversal, extent tree parsing (`ext4_extent_header`, `ext4_extent`), and deleted inode identification.

### 2.4 `crates/file-carving`
- Format-aware header/tail validation and chunk boundary detection:
  - **Images:** JPEG (`FF D8 FF` SOI, marker stream validation, `FF D9` EOI), PNG (`89 50 4E 47 0D 0A 1A 0A`, chunk CRC32 verification, `IEND`), GIF (`GIF87a`/`GIF89a`, trailer `3B`), WebP (`RIFF....WEBP`), BMP (`42 4D`, header size calculation), TIFF (`49 49 2A 00` / `4D 4D 00 2A`).
  - **Documents:** PDF (`%PDF-`, xref/trailer parsing, `%%EOF`), TXT (UTF-8/ASCII heuristic analysis), ZIP / Office OpenXML (`PK\x03\x04` header, central directory, `PK\x05\x06` EOCD validation for DOCX, XLSX, PPTX).
  - **Media:** MP3 (ID3v2 header and sync frame headers `FFF`), WAV (`RIFF....WAVEfmt `), MP4 / MOV (`ftyp` atom traversal and box length parsing), MKV (`\x1A\x45\xDF\xA3` EBML header).

### 2.5 `crates/scanner`
- **Quick Scan:** Mounts filesystem structures from selected source and enumerates unallocated/deleted directory entries. Preserves original filenames, timestamps, and directories.
- **Deep Scan:** Linear or block-strided scan of unallocated space or entire disk using `crates/file-carving`. Bounded memory buffers (e.g. 1MB ring buffer), multi-threaded block analysis via `rayon`, real-time progress events, cancellation support.

### 2.6 `crates/file-preview`
- Untrusted sandbox preview generator:
  - Generates thumbnail base64 data for images without invoking external viewers.
  - Produces formatted Hex + ASCII dumps for any raw recovered file or carved block.
  - Extracts document metadata (author, title, page count) safely.

### 2.7 `crates/image-repair`
- Non-destructive image repair on copies:
  - Detects truncated JPEG files and inserts correct `FF D9` EOI markers.
  - Detects corrupted PNG chunk CRCs and recalculates valid CRC checksums.
  - Identifies misnamed files by comparing magic byte signatures with file extensions.
  - Leaves original recovered files completely untouched, writing repaired files as `*.repaired.ext`.

### 2.8 `crates/reporting`
- Generates comprehensive local recovery audit logs in JSON, Markdown, and CSV formats, detailing source device, scan duration, recovered byte counts, SHA-256 hashes, and integrity classification.

---

## 3. Concurrency & Resource Bounding

- **Memory Bounding:** The deep scan operates on a bounded sliding window buffer. File carving holds candidate chunks up to a configurable maximum file size limit (default 100 MB for images, 2 GB for video) to prevent out-of-memory denial of service.
- **Thread Management:** Scanning uses worker pools with cooperative cancellation checking at every block boundary.
- **I/O Safety:** Device access handles are opened exclusively in read-only shared mode. Writing to the source device is prohibited at the API design level.
