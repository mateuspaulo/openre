use clap::{Parser, Subcommand};
use device_access::{create_disk_image, enumerate_devices, FileBlockReader};
use file_preview::generate_preview;
use image_repair::repair_image;
use recovery_core::CancellationToken;
use scanner::Scanner;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "openre-cli")]
#[command(about = "OpenRe — Professional, Open-Source, Privacy-First File Recovery", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List detected storage devices, partitions, and volumes
    ListDevices,

    /// Scan a drive, partition, or disk image for deleted/carved files
    Scan {
        /// Source device, drive letter, or disk image file (e.g. C:, \\.\PhysicalDrive0, disk.img)
        #[arg(short, long)]
        source: String,

        /// Scan mode: 'quick' (filesystem metadata) or 'deep' (block-strided carving)
        #[arg(short, long, default_value = "deep")]
        mode: String,

        /// Optional output directory to automatically recover discovered files
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Inspect and attempt non-destructive repair on a corrupted JPEG or PNG image
    Repair {
        /// Path to the damaged file
        #[arg(short, long)]
        file: PathBuf,

        /// Destination path for the repaired copy
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Inspect and print hex dump / metadata sandbox preview for a file
    Preview {
        /// Path to the file to inspect
        #[arg(short, long)]
        file: PathBuf,
    },

    /// Clone a drive or volume to a raw disk image (.img/.dd) with bad sector tracking
    ImageDisk {
        /// Source device or drive
        #[arg(short, long)]
        source: String,

        /// Target image file path (must not be on the source drive)
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ListDevices => {
            println!("==> OpenRe — Enumerating Connected Devices");
            let devices = enumerate_devices();
            if devices.is_empty() {
                println!("No devices detected.");
            } else {
                for dev in devices {
                    println!(
                        "- [{:?}] {} (ID: {}) — Elevation needed: {}",
                        dev.device_type, dev.name, dev.id, dev.requires_elevation
                    );
                }
            }
        }
        Commands::Scan { source, mode, output } => {
            println!("==> OpenRe — Scanning Source: {}", source);
            let path = PathBuf::from(&source);
            let mut reader = FileBlockReader::open(&path)?;
            let cancel = CancellationToken::new();

            let files = if mode.eq_ignore_ascii_case("quick") {
                Scanner::quick_scan(&mut reader, &cancel, |p| {
                    print!("\r[Quick Scan] {} ... Found: {}", p.current_label, p.files_found);
                    let _ = std::io::stdout().flush();
                })?
            } else {
                Scanner::deep_scan(&mut reader, &cancel, |p| {
                    print!(
                        "\r[Deep Scan] {} | Progress: {} / {} bytes | Files: {}",
                        p.current_label, p.bytes_scanned, p.total_bytes, p.files_found
                    );
                    let _ = std::io::stdout().flush();
                })?
            };

            println!("\n==> Discovered {} files:", files.len());
            for (idx, f) in files.iter().enumerate().take(20) {
                println!(
                    "  [{}] {} ({:.2} KB) — Integrity: {}",
                    idx + 1,
                    f.filename,
                    f.size_bytes as f64 / 1024.0,
                    f.integrity.as_str()
                );
            }
            if files.len() > 20 {
                println!("  ... and {} more files.", files.len() - 20);
            }

            if let Some(dest_dir) = output {
                println!("\n==> Recovering files to {:?}", dest_dir);
                let report = Scanner::recover_files(
                    &mut reader,
                    &files,
                    &dest_dir,
                    &cancel,
                    |file, done, total| {
                        println!("  [{}/{}] Saved: {}", done, total, file.filename);
                    },
                )?;

                let report_path = dest_dir.join("recovery_report.md");
                let mut rf = File::create(&report_path)?;
                rf.write_all(report.to_markdown().as_bytes())?;
                println!("==> Audit report saved to {:?}", report_path);
            }
        }
        Commands::Repair { file, output } => {
            println!("==> OpenRe — Image Repair: {:?}", file);
            let data = std::fs::read(&file)?;
            let fname = file.file_name().unwrap_or_default().to_string_lossy();
            let report = repair_image(&data, &fname)?;

            println!("Format Detected: {}", report.detected_format);
            println!("Issues Identified: {:?}", report.issues_found);
            println!("Fixes Applied: {:?}", report.fixes_applied);

            if report.success {
                std::fs::write(&output, &report.repaired_data)?;
                println!("==> Repaired copy successfully saved to {:?}", output);
            } else {
                println!("No structural fixes were required or possible.");
            }
        }
        Commands::Preview { file } => {
            println!("==> OpenRe — File Preview & Hex Dump: {:?}", file);
            let data = std::fs::read(&file)?;
            let ext = file.extension().unwrap_or_default().to_string_lossy();
            let preview = generate_preview(&data, &ext);

            println!("Preview Type: {}", preview.preview_type);
            for (k, v) in &preview.metadata_entries {
                println!("  {}: {}", k, v);
            }

            println!("\n--- Hex Dump (First 512 bytes) ---");
            for line in preview.hex_dump.iter().take(16) {
                println!("{}  {} |{}|", line.offset, line.hex, line.ascii);
            }
        }
        Commands::ImageDisk { source, output } => {
            println!("==> OpenRe — Creating Raw Disk Image from {}", source);
            let path = PathBuf::from(&source);
            let mut reader = FileBlockReader::open(&path)?;
            let cancel = CancellationToken::new();

            let report = create_disk_image(&mut reader, &output, &cancel, |done, total| {
                print!("\r[Imaging] Copied: {} / {} bytes", done, total);
                let _ = std::io::stdout().flush();
            })?;

            println!(
                "\n==> Imaging Complete: {} bytes copied, {} bad sectors logged.",
                report.copied_bytes, report.bad_sectors_count
            );
        }
    }

    Ok(())
}
