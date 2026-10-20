# Threat Model & Security Architecture — OpenRe

**Document Version:** 1.0.0  
**Framework:** STRIDE-based Threat Modeling for Digital Forensics & File Recovery  
**Target Architecture:** Rust 2021 Workspace + Tauri v2 Desktop Client  

---

## 1. System Assets & Trust Boundaries

### 1.1 Assets
1. **Source Storage Devices:** Physical hard drives (SATA, NVMe, USB), memory cards (SD), logical volumes, partitions, and disk images containing critical, lost, or failing user data.
2. **Recovered User Files:** Salvaged photos, office documents, financial spreadsheets, videos, and private records.
3. **Target Destination Storage:** The secondary media where recovered files are safely exported.
4. **Host Operating System Integrity:** Protecting kernel, memory, and filesystem stability during low-level raw disk operations.

### 1.2 Trust Boundaries
- **Boundary 1: Host vs. Source Media.** All raw data read from target disks, corrupted filesystems, and unpartitioned space must be treated as **completely untrusted**. Attackers could deliberately format drives with malicious structures (e.g., malformed MFT records, corrupted ELF/PE headers, zip bombs) to exploit recovery software.
- **Boundary 2: Recovery Core vs. File Preview.** The file preview subsystem renders recovered images, text, and documents. Previewing must never execute native code, shell scripts, or document macros.
- **Boundary 3: Recovery Core vs. Target Filesystem.** Output file extraction must prevent path traversal attacks (`../../`) and accidental overwriting of host system files.

---

## 2. Threat Analysis & Mitigations (STRIDE)

| Threat Category | Potential Attack Vector | OpenRe Architectural Mitigation |
| :--- | :--- | :--- |
| **Spoofing** | Malicious device purporting to be a safe block volume. | Strict read-only device handles; permission checks before issuing disk geometry IOCTLs. |
| **Tampering** | Accidental or malicious write operations to the source media during scanning or recovery. | **Enforced Read-Only Mode.** Device handles on Windows are opened with `GENERIC_READ` and `FILE_SHARE_READ \| FILE_SHARE_WRITE`. Linux handles use `O_RDONLY`. Writes to source devices are architecturally impossible in normal scanning. Source drive and destination drive must be distinct. |
| **Repudiation** | Conflicting claims regarding data integrity or recovery success. | Audit reports with SHA-256 integrity hashes, byte-for-byte verification records, and clear status labels (`Likely recoverable`, `Corrupted`). |
| **Information Disclosure** | Data leakage over network or local temp files. | Strict offline-first architecture; zero network code in recovery crates; no telemetry; thumbnails kept in transient memory. |
| **Denial of Service** | Malformed headers causing infinite loops, recursive directory traversal, or memory exhaustion (e.g., decompression bombs, zip bombs). | Bounded memory allocations; streaming chunk parsers with strict recursion limits; timeout-governed preview generators; memory-safe Rust bounds checking. |
| **Elevation of Privilege** | Exploit payload embedded in recovered file executing during preview or path traversal during extraction. | Untrusted sandbox preview (no script execution, no macro invocation, purely static decoding); strict path sanitization stripping illegal characters and directory traversal tokens (`..`, `/`, `\`). |

---

## 3. Path Traversal & Destination Guardrails

When recovering files with recovered filesystem directory paths (e.g., from NTFS or FAT32 directory tables):
1. **Canonicalization:** The destination folder path is resolved to an absolute canonical path.
2. **Sanitization:** Every directory segment and filename is stripped of control characters, `..`, null bytes, and path separator sequences.
3. **Enclosed Boundary Assertion:** The engine asserts that the final output file path is strictly a descendant of the configured target directory (`starts_with(&dest_dir)`).
4. **Collision Avoidance:** Existing files are never overwritten silently. When collision occurs, unique numeric suffixes (`file_1.jpg`) are assigned.
5. **Physical Device Separation:** OpenRe checks volume mount points and drive geometry to alert and block users from saving recovered files onto the source drive being scanned.

---

## 4. Antivirus False Positive Management

Digital recovery tools frequently get flagged by heuristic antivirus scanners due to raw sector reading APIs (`CreateFileW` with `\\.\PhysicalDrive` or direct disk access).

OpenRe mitigates this by:
1. Publishing clean, uncompressed, un-packed binaries directly built on GitHub Actions CI.
2. Providing transparent source code and reproducible build instructions.
3. Supplying official SHA-256 checksums for all release binaries.
4. Avoiding suspicious techniques: no code injection, no process concealment, no persistence hooks, no evasion behaviors.
