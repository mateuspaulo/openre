# OpenRe Recovery Engine — Detailed Technical Specification

**Target Subsystems:** Filesystem Analysis, Block-Based Carving, Integrity Evaluation, Non-Destructive Repair  

---

## 1. Quick Scan vs. Deep Scan Mechanics

```
                         ┌─────────────────────────────┐
                         │      User Selects Media     │
                         └──────────────┬──────────────┘
                                        │
                    ┌───────────────────┴───────────────────┐
                    │                                       │
            [ Quick Scan ]                           [ Deep Scan ]
                    │                                       │
      ┌─────────────▼─────────────┐           ┌─────────────▼─────────────┐
      │  Filesystem Parser        │           │  Block Streaming Engine   │
      │  (NTFS MFT / FAT32 Dir /  │           │  (Bounded 1MB Ring Buffer │
      │   exFAT / ext4 Inodes)    │           │   Sector-aligned reads)   │
      └─────────────┬─────────────┘           └─────────────┬─────────────┘
                    │                                       │
      ┌─────────────▼─────────────┐           ┌─────────────▼─────────────┐
      │  Deleted Record Detector  │           │  Format-Aware Carvers     │
      │  - NTFS Record 0x00 Flag  │           │  - Magic Byte Matching    │
      │  - FAT32 0xE5 Deleted Tag │           │  - Chunk Tree Validation  │
      │  - ext4 Inode Dealloc     │           │  - EOI / EOCD Boundaries  │
      └─────────────┬─────────────┘           └─────────────┬─────────────┘
                    │                                       │
                    └───────────────────┬───────────────────┘
                                        │
                         ┌──────────────▼──────────────┐
                         │   Integrity Classification  │
                         │   - Likely Recoverable      │
                         │   - Partially Recoverable   │
                         │   - Corrupted / Unknown     │
                         └──────────────┬──────────────┘
                                        │
                         ┌──────────────▼──────────────┐
                         │  Safe Output Extraction     │
                         │  (Distinct media check,     │
                         │   collision-safe renaming)  │
                         └─────────────────────────────┘
```

---

## 2. Filesystem Parsers

### 2.1 NTFS Master File Table (MFT) Analysis
1. **Boot Sector:** Reads sector 0. Validates OEM ID `NTFS    `. Extracts bytes per sector (typically 512), sectors per cluster (e.g. 8 = 4096 bytes), and clusters per MFT record (negative value e.g. -10 represents $2^{10} = 1024$ bytes).
2. **MFT Locating:** Calculates byte offset of `$MFT` = `MFT_Cluster * Sectors_Per_Cluster * Bytes_Per_Sector`.
3. **Record Parsing:**
   - Evaluates 1024-byte record header. Signature must be `FILE` (`0x454C4946`).
   - Checks record flags at offset `0x16`:
     - `0x00`: Deleted file (unallocated record).
     - `0x01`: Active file.
     - `0x02`: Deleted directory.
     - `0x03`: Active directory.
   - Applies update sequence fixups (USN verification).
   - Iterates attributes until terminator `0xFFFFFFFF`:
     - `$STANDARD_INFORMATION` (`0x10`): Extracts 64-bit FILETIME creation, modification, and access timestamps.
     - `$FILE_NAME` (`0x30`): Extracts parent directory reference, UTF-16 filename length, and filename string.
     - `$DATA` (`0x80`):
       - If **Resident** (flag `0x00`): extracts file content directly from record body.
       - If **Non-Resident** (flag `0x01`): decodes compression runlists (variable-length cluster run pairs: cluster count and starting cluster LCN).
4. **Deleted File Extraction:** Even when marked deleted (`0x00`), the runlists often remain intact until overwritten by new files. The engine reads these clusters and checks for content integrity.

### 2.2 FAT32 Directory Entry Analysis
1. **BPB Analysis:** Validates `0x55AA` signature at offset 510. Extracts `bytes_per_sector`, `sectors_per_cluster`, `reserved_sectors`, `num_fats`, `fat_size_32`, and `root_cluster`.
2. **Directory Table Walk:** Each 32-byte directory entry is inspected:
   - First byte `0xE5`: Marks a deleted file entry.
   - First byte `0x00`: End of directory.
   - Attribute `0x0F`: Long File Name (LFN) entry. LFN entries are collected in reverse sequence to reconstruct full Unicode filenames.
   - Standard Entry: Extracts short filename, extension, file attributes, creation/modification timestamps, high cluster number (`offset 0x14`) and low cluster number (`offset 0x1A`), and 32-bit file size (`offset 0x1C`).
3. **Cluster Allocation Assessment:** Cross-references the starting cluster with the FAT table. If clusters have not been allocated to a new file chain, the file data can be extracted directly.

---

## 3. Deep Carving & Format-Aware Boundary Detection

Carving searches raw block boundaries (default 512-byte or 4096-byte alignment) for file signatures and confirms valid termination:

### 3.1 JPEG / JPG
- **Header:** `FF D8 FF` (SOI followed by marker prefix).
- **Marker Traversal:** Walks markers (`FF E0` APP0, `FF E1` EXIF, `FF DB` DQT, `FF C0` SOF0, `FF C4` DHT, `FF DA` SOS).
- **Scan Data & Termination:** Scans entropy-coded data until `FF D9` (EOI).
- **Integrity Check:** If `FF D9` is found without premature truncation, marked `Likely recoverable`. If truncated, marked `Partially recoverable`.

### 3.2 PNG
- **Header:** `89 50 4E 47 0D 0A 1A 0A`.
- **Chunk Traversal:** Loops through chunks (`[4 bytes length][4 bytes type][length bytes data][4 bytes CRC32]`).
- **Validation:** Verifies CRC32 of type + data against the stored CRC.
- **Termination:** Stops on `IEND` chunk. Total file length is exactly calculated.

### 3.3 PDF
- **Header:** `%PDF-1.x`.
- **Termination:** Scans for `%%EOF` marker, cross-reference table (`xref`), and `trailer` dictionary.

### 3.4 ZIP / Microsoft Office OpenXML (DOCX, XLSX, PPTX)
- **Local Header:** `50 4B 03 04` (`PK\x03\x04`).
- **End of Central Directory (EOCD):** Searches backwards from candidate boundary for `50 4B 05 06` (`PK\x05\x06`).
- **Office Classification:** Inspects zip directory entries for `[Content_Types].xml`, `word/document.xml`, `xl/workbook.xml`, or `ppt/presentation.xml` to differentiate DOCX/XLSX/PPTX from generic ZIP archives.

---

## 4. Integrity Scoring Model

OpenRe assigns integrity status through deterministic validation rules:

| Classification | Meaning | Validation Conditions |
| :--- | :--- | :--- |
| **Likely Recoverable** | High probability of full recovery | Valid header, valid internal chunk/marker sequence, valid termination marker (e.g. `FF D9`, `IEND`, `%%EOF`), and cluster allocation non-overwritten. |
| **Partially Recoverable** | File body present but truncated or missing trailer | Valid header present, expected structural blocks detected, but premature EOF or minor CRC error encountered. File repair may salvage partial rendering. |
| **File Structure Detected; Integrity Unverified** | Metadata or raw header located | Signature found, but file type does not specify a deterministic length or internal checksum (e.g. raw text, stream). |
| **Corrupted** | Severe data corruption or overwriting | MFT record exists but pointed clusters contain zeroes, or archive central directory table fails to decompress. |
| **Unknown** | Unclassified binary block | Detected through heuristic entropy analysis or unrecognized format. |
