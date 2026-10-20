# Privacy-First Architecture & Design — OpenRe

**Policy Status:** Strict Zero-Knowledge, Offline-Only Architecture  
**Enforcement:** Architectural separation and automated CI network isolation checks  

---

## 1. Core Privacy Principles

Data recovery inherently touches user data of the highest sensitivity: deleted personal documents, family photographs, private financial records, credentials, and deleted system logs.

OpenRe operates under a mandatory **Zero-Telemetry, Completely Offline** guarantee:
1. **Zero External Network Connections:** OpenRe contains no analytics libraries, no crash reporting daemons, no telemetry beacons, no advertising frameworks, and no background update pingers in its recovery engine.
2. **Local-Only Processing:** Every byte of disk scanning, filesystem parsing, file carving, thumbnail generation, hex inspection, and file repair runs strictly within local memory and local CPU on the user's computer.
3. **Never Upload or Transmit:**
   - Disk or partition raw bytes.
   - Recovered document or media contents.
   - Original file names, directory structures, or file sizes.
   - Hardware identifiers (MAC addresses, disk serial numbers, motherboard UUIDs).
   - EXIF location/camera metadata.
   - File hashes or checksums.
   - Diagnostic crash dumps or system logs.
4. **No Mandatory Account or Cloud Gateway:** OpenRe never requires registration, email addresses, license activation keys, or internet authorization.

---

## 2. Local Data Handling & Storage

- **In-Memory Buffering:** Block reads from physical disks and disk images use bounded, circular in-memory buffers (typically 64 KB to 4 MB chunks). These buffers are cleared and deallocated upon session termination.
- **Session Cache (Optional):** When users choose to persist a long-running deep scan session, data is stored in a local SQLite file (`openre_session.db`) residing inside the user's application data directory or chosen output directory. This database remains strictly local and unencrypted only because it is local; users can delete it at any time.
- **Thumbnails & Previews:** Image thumbnails generated for preview are rendered into local memory or ephemeral canvas objects and are never cached to external temp directories accessible to other applications.

---

## 3. Automated Network Isolation Verification

To guarantee that OpenRe cannot regress into making accidental or unauthorized network calls:
1. **Crate Boundary Segregation:** The `recovery-core`, `filesystem`, `scanner`, `file-carving`, `file-preview`, and `image-repair` crates explicitly exclude network dependencies (such as `reqwest`, `hyper`, `tokio::net`, `curl`, `ureq`).
2. **Automated Security Test:** An automated integration test in `tests/security/test_network_isolation.rs` inspects the crate dependency trees and verifies that no network sockets can be opened during recovery operations.
3. **CI Pipeline Audit:** Continuous integration runs `cargo tree` scans and security audit tools to fail builds if unapproved networking crates are introduced into recovery modules.

---

## 4. Updates & External Communications

- OpenRe does not implement automatic background update downloads.
- Future update notification capabilities, if introduced in the UI layer, must require explicit user opt-in, use transparent HTTPS requests directly to official GitHub Release API endpoints, and remain completely separated from the recovery engine.

---

## 5. Security & Antivirus Transparency

Because OpenRe performs raw disk and sector reading, security software may inspect its behavior. OpenRe maintains absolute transparency:
- Fully public, auditable open-source repository.
- Reproducible, verifiable builds with published SHA-256 checksums.
- Clear, un-obfuscated binaries without packers or evasion techniques.
