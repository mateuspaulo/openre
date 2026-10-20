# Troubleshooting & Frequently Asked Questions — OpenRe

---

## 1. Common Errors & Solutions

### "Access Denied" or "Insufficient Privileges" when scanning a drive
- **Cause:** Operating systems restrict unprivileged user processes from reading raw sector blocks of physical drives (`\\.\PhysicalDrive0` on Windows, `/dev/sda` on Linux).
- **Solution:** 
  - On Windows: Right-click `OpenRe.exe` and select **Run as Administrator**.
  - On Linux: Launch with `sudo openre` or add your user to the `disk` group.
  - Alternatively: If you only need to recover from an unencrypted folder or a disk image (`.img`, `.dd`), select **Scan a Folder** or **Scan a Disk Image** in OpenRe, which runs without administrative elevation.

### "Destination is on the source device" Warning
- **Cause:** OpenRe detected that the chosen output directory resides on the same physical drive being recovered.
- **Why this is blocked:** Saving recovered files back onto the same drive overwrites unallocated blocks, permanently destroying the very files you are trying to salvage!
- **Solution:** Connect an external USB drive, memory stick, or select a different physical partition as the destination.

### "Recovered image cannot be opened"
- **Cause:** The file was partially overwritten before recovery, resulting in truncated data or missing trailer markers.
- **Solution:** Use OpenRe's built-in **Repair Corrupted Image** tool. It will inspect headers, recalculate chunk checksums, insert missing markers (e.g. JPEG `FF D9` EOI), and output a repaired copy.

### "Files found but with generic names like `carved_000123.jpg`"
- **Cause:** Deep Scan (file carving) was used. Carving searches raw byte signatures when the filesystem table (FAT/MFT) is destroyed or formatted. Carving reconstructs the file contents, but the original filesystem directory tree and original filename are not stored in the file data itself.
- **Solution:** When possible, run **Quick Scan** first. Quick Scan parses intact filesystem records to preserve original names, directories, and timestamps.
