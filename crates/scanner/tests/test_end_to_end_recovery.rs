use device_access::FileBlockReader;
use file_preview::generate_preview;
use image_repair::repair_image;
use recovery_core::{CancellationToken, IntegrityStatus};
use scanner::Scanner;
use std::fs::File;
use std::io::Write;

#[test]
fn test_end_to_end_synthetic_recovery_pipeline() {
    let temp_dir = std::env::temp_dir().join(format!("openre_test_{}", std::time::SystemTime::now().elapsed().unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let disk_image_path = temp_dir.join("synthetic_disk.img");
    let recovery_out_dir = temp_dir.join("recovered_output");

    // 1. Create a 64 KB synthetic disk image with multiple file signatures embedded at sector boundaries
    let mut disk_data = vec![0u8; 65536];

    // Sector 1 (offset 512): Valid JPEG
    let valid_jpeg = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
        0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08,
        0xFF, 0xD9, // EOI
    ];
    disk_data[512..512 + valid_jpeg.len()].copy_from_slice(&valid_jpeg);

    // Sector 4 (offset 2048): Truncated JPEG (missing EOI)
    let truncated_jpeg = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00,
    ];
    disk_data[2048..2048 + truncated_jpeg.len()].copy_from_slice(&truncated_jpeg);

    // Sector 8 (offset 4096): Minimal PDF
    let valid_pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n%%EOF\n";
    disk_data[4096..4096 + valid_pdf.len()].copy_from_slice(valid_pdf);

    // Write synthetic disk image to file
    {
        let mut f = File::create(&disk_image_path).unwrap();
        f.write_all(&disk_data).unwrap();
    }

    // 2. Open FileBlockReader on the synthetic disk image
    let mut reader = FileBlockReader::open(&disk_image_path).expect("Should open synthetic image");
    let cancel = CancellationToken::new();

    // 3. Run Deep Scan
    let discovered = Scanner::deep_scan(&mut reader, &cancel, |_p| {}).expect("Deep scan should run");
    for f in &discovered {
        eprintln!("Discovered file: {} at offset {} (ext: {}, integrity: {:?})", f.filename, f.source_offset, f.extension, f.integrity);
    }
    assert!(discovered.len() >= 3, "Expected at least 3 files carved from synthetic disk, found {}", discovered.len());

    // Verify discovered file types
    let has_jpg = discovered.iter().any(|f| f.extension == "jpg" && f.integrity == IntegrityStatus::LikelyRecoverable);
    let has_trunc_jpg = discovered.iter().any(|f| f.extension == "jpg" && f.integrity == IntegrityStatus::PartiallyRecoverable);
    let has_pdf = discovered.iter().any(|f| f.extension == "pdf");

    assert!(has_jpg, "Should discover valid JPEG");
    assert!(has_trunc_jpg, "Should discover truncated JPEG");
    assert!(has_pdf, "Should discover PDF");

    // 4. Test pre-recovery preview on the first JPEG
    let preview = generate_preview(&valid_jpeg, "jpg");
    assert_eq!(preview.hex_dump.len(), 3);
    assert!(preview.hex_dump[0].hex.starts_with("FF D8 FF E0"));

    // 5. Test Non-Destructive Repair on the truncated JPEG
    let repair_report = repair_image(&truncated_jpeg, "photo.jpg").expect("Repair should succeed");
    assert!(repair_report.success);
    assert_eq!(&repair_report.repaired_data[repair_report.repaired_data.len() - 2..], &[0xFF, 0xD9]);

    // 6. Recover all discovered files to output directory
    let recovery_report = Scanner::recover_files(
        &mut reader,
        &discovered,
        &recovery_out_dir,
        &cancel,
        |_f, _done, _total| {},
    ).expect("Recovery should succeed");

    assert_eq!(recovery_report.total_files_discovered, discovered.len());
    assert_eq!(recovery_report.total_files_recovered, discovered.len());

    // 7. Verify audit report generation
    let md = recovery_report.to_markdown();
    assert!(md.contains("# OpenRe — Recovery Audit Report"));
    assert!(md.contains("synthetic_disk.img"));

    let json = recovery_report.to_json().unwrap();
    assert!(json.contains("carved_"));

    // 8. Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
