# Testing Guide — OpenRe

**Scope:** Unit Tests, Integration Tests, Forensic Fixtures, Security & Network Isolation  

---

## 1. Running the Automated Test Suite

OpenRe includes tests spanning all recovery modules, filesystem parsers, format-aware carvers, and privacy assertions:

```bash
# Run all unit and integration tests across the workspace
cargo test

# Run tests with detailed console output
cargo test -- --nocapture

# Run only file carving tests
cargo test -p file-carving

# Run only filesystem parser tests
cargo test -p filesystem

# Run network isolation security audit
cargo test -p recovery-core --test test_network_isolation
```

---

## 2. Test Fixture Strategy (100% Synthetic)

**Mandatory Rule:** Personal files, real user disks, or copyrighted images must NEVER be used as test fixtures.

All fixtures located in `tests/fixtures/` are generated programmatically using synthetic data:
- `synthetic_fat32.bin`: Minimal valid FAT32 volume with deleted directory entries.
- `synthetic_ntfs_mft.bin`: Synthetic 1024-byte MFT record with unallocated status (`0x00`) and standard attributes.
- `corrupted_sample.jpg`: Programmatically generated JPEG with valid markers but missing `FF D9` EOI to test image repair.
- `valid_sample.png`: Synthetic 16x16 PNG with valid chunk CRCs.
- `truncated_sample.png`: Synthetic PNG missing the `IEND` chunk.
- `sample_document.docx`: Minimal valid OpenXML ZIP archive containing synthetic text.

---

## 3. Security & Privacy Testing

OpenRe includes an automated network isolation test verifying:
1. No crate in the recovery workspace imports networking or HTTP client dependencies.
2. The recovery engine does not attempt to bind to local sockets or issue outbound network syscalls during scanning.
3. Path traversal attack strings (`../../../../etc/passwd` or `..\..\..\Windows\System32\cmd.exe`) in recovered metadata are safely neutralized.
