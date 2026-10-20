#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use device_access::{enumerate_devices, open_block_reader};
use file_preview::generate_preview;
use image_repair::repair_image;
use recovery_core::{CancellationToken, RecoveredFile, RecoveryError};
use scanner::Scanner;
use serde::Deserialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[derive(Debug, Deserialize)]
struct IpcMessage {
    cmd: String,
    #[serde(rename = "msgId")]
    msg_id: Option<String>,
    payload: serde_json::Value,
}

#[derive(Debug)]
enum UserEvent {
    EvaluateScript(String),
}

struct AppState {
    cancel_token: CancellationToken,
    last_discovered: Vec<RecoveredFile>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("OpenRe — Professional File Recovery")
        .with_inner_size(LogicalSize::new(1240.0, 840.0))
        .with_min_inner_size(LogicalSize::new(960.0, 640.0))
        .build(&event_loop)?;

    let app_state = Arc::new(Mutex::new(AppState {
        cancel_token: CancellationToken::new(),
        last_discovered: Vec::new(),
    }));

    let html_content = include_str!("../../dist/index.html");

    let proxy_for_ipc = proxy.clone();
    let state_for_ipc = Arc::clone(&app_state);

    let webview = WebViewBuilder::new()
        .with_html(html_content)
        .with_ipc_handler(move |request| {
            let msg_str = request.body();
            let msg: Result<IpcMessage, _> = serde_json::from_str(msg_str);
            if let Ok(msg) = msg {
                handle_ipc_message(&msg, &state_for_ipc, &proxy_for_ipc);
            }
        })
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                let _ = &webview;
            }
            Event::UserEvent(UserEvent::EvaluateScript(script)) => {
                let _ = webview.evaluate_script(&script);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => (),
        }
    });
}

fn handle_ipc_message(
    msg: &IpcMessage,
    state: &Arc<Mutex<AppState>>,
    proxy: &EventLoopProxy<UserEvent>,
) {
    let msg_id = msg.msg_id.clone();
    let cmd = msg.cmd.as_str();

    match cmd {
        "list_devices" => {
            let devs = enumerate_devices();
            let json = serde_json::to_string(&devs).unwrap_or_else(|_| "[]".to_string());
            emit_log("info", &format!("Enumerated {} available storage devices and volumes", devs.len()), proxy);
            emit_response(msg_id, &json, proxy);
        }
        "select_folder" => {
            let folder = rfd::FileDialog::new().pick_folder();
            let path_str = folder.map(|p| p.to_string_lossy().to_string());
            if let Some(ref p) = path_str {
                emit_log("info", &format!("Custom storage directory selected: {}", p), proxy);
            }
            let json = serde_json::to_string(&path_str).unwrap_or_else(|_| "null".to_string());
            emit_response(msg_id, &json, proxy);
        }
        "select_image_file" => {
            let file = rfd::FileDialog::new()
                .add_filter("Raw Disk Images", &["img", "dd", "raw", "bin", "iso"])
                .pick_file();
            let path_str = file.map(|p| p.to_string_lossy().to_string());
            if let Some(ref p) = path_str {
                emit_log("info", &format!("Raw disk image selected: {}", p), proxy);
            }
            let json = serde_json::to_string(&path_str).unwrap_or_else(|_| "null".to_string());
            emit_response(msg_id, &json, proxy);
        }
        "start_scan" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let mode = msg.payload.get("mode").and_then(|v| v.as_str()).unwrap_or("quick").to_string();

            emit_log("info", &format!("Initializing {} scan on target '{}'...", mode.to_uppercase(), source_id), proxy);

            let cancel = CancellationToken::new();
            if let Ok(mut st) = state.lock() {
                st.cancel_token = cancel.clone();
            }

            let proxy_clone = proxy.clone();
            let state_clone = Arc::clone(state);

            thread::spawn(move || {
                let mut reader = match open_block_reader(&source_id) {
                    Ok(r) => r,
                    Err(e) => {
                        let err_msg = format!("Failed to open target '{}': {}", source_id, e);
                        emit_log("error", &err_msg, &proxy_clone);
                        if let RecoveryError::PermissionDenied(_) = e {
                            emit_log(
                                "warn",
                                "Administrator privileges required to access raw disk sectors. Right-click OpenRe.exe and choose 'Run as Administrator', or choose a specific directory to scan without elevation.",
                                &proxy_clone,
                            );
                        }
                        emit_event(
                            "scan_error",
                            &serde_json::json!({ "error": err_msg }).to_string(),
                            &proxy_clone,
                        );
                        return;
                    }
                };

                let total_bytes = reader.total_bytes();
                emit_log(
                    "info",
                    &format!(
                        "Target device opened successfully ({}). Media capacity: {:.2} MB ({} bytes)",
                        reader.device_id(),
                        total_bytes as f64 / (1024.0 * 1024.0),
                        total_bytes
                    ),
                    &proxy_clone,
                );

                if source_id.len() <= 3 && source_id.contains(':') && !reader.device_id().starts_with(r"\\.\") {
                    emit_log(
                        "info",
                        "Notice: Raw sector carving requires Administrator elevation. Deep scan running in file-stream mode. (To scan raw unallocated sectors, run OpenRe as Administrator).",
                        &proxy_clone,
                    );
                }

                let proxy_progress = proxy_clone.clone();
                let proxy_log = proxy_clone.clone();
                let last_progress_emit = Arc::new(Mutex::new(std::time::Instant::now()));
                let last_log_time = Arc::new(Mutex::new(std::time::Instant::now()));
                let last_files_count = Arc::new(Mutex::new(0usize));

                let files_res = if mode == "quick" {
                    emit_log("info", "Quick scan started: inspecting filesystem structures and directory entries...", &proxy_clone);
                    let emit_tracker = Arc::clone(&last_progress_emit);
                    Scanner::quick_scan(&mut *reader, &cancel, move |p| {
                        let should_emit = p.is_complete || {
                            if let Ok(mut last_t) = emit_tracker.lock() {
                                if last_t.elapsed().as_millis() >= 100 {
                                    *last_t = std::time::Instant::now();
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        };
                        if should_emit {
                            emit_event("scan_progress", &serde_json::to_string(&p).unwrap(), &proxy_progress);
                        }
                    })
                } else {
                    emit_log("info", "Deep scan started: scanning contiguous block windows and analyzing file signatures...", &proxy_clone);
                    let emit_tracker = Arc::clone(&last_progress_emit);
                    Scanner::deep_scan(&mut *reader, &cancel, move |p| {
                        let should_emit = p.is_complete || {
                            if let Ok(mut last_t) = emit_tracker.lock() {
                                if last_t.elapsed().as_millis() >= 100 {
                                    *last_t = std::time::Instant::now();
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        };
                        if should_emit {
                            emit_event("scan_progress", &serde_json::to_string(&p).unwrap(), &proxy_progress);
                        }

                        if let Ok(mut last_t) = last_log_time.lock() {
                            if last_t.elapsed().as_secs() >= 3 {
                                *last_t = std::time::Instant::now();
                                let pct = if p.total_bytes > 0 { (p.bytes_scanned as f64 / p.total_bytes as f64) * 100.0 } else { 0.0 };
                                emit_log(
                                    "info",
                                    &format!(
                                        "Deep Scan progress: {:.1}% complete ({:.1} MB/s) | {} files discovered",
                                        pct,
                                        p.speed_bytes_per_sec as f64 / (1024.0 * 1024.0),
                                        p.files_found
                                    ),
                                    &proxy_log,
                                );
                            }
                        }

                        if let Ok(mut last_c) = last_files_count.lock() {
                            if p.files_found > *last_c {
                                emit_log(
                                    "success",
                                    &format!("Discovered valid signature! Total recoverable files: {}", p.files_found),
                                    &proxy_log,
                                );
                                *last_c = p.files_found;
                            }
                        }
                    })
                };

                match files_res {
                    Ok(files) => {
                        let count = files.len();
                        if let Ok(mut st) = state_clone.lock() {
                            st.last_discovered = files.clone();
                        }
                        emit_log(
                            "success",
                            &format!("Scan completed successfully! {} recoverable files identified.", count),
                            &proxy_clone,
                        );
                        emit_event(
                            "scan_completed",
                            &serde_json::json!({ "files": files }).to_string(),
                            &proxy_clone,
                        );
                    }
                    Err(RecoveryError::Cancelled) => {
                        emit_log("warn", "Scan operation cancelled by user.", &proxy_clone);
                        emit_event(
                            "scan_error",
                            &serde_json::json!({ "error": "Scan operation cancelled by user." }).to_string(),
                            &proxy_clone,
                        );
                    }
                    Err(e) => {
                        let err_msg = format!("Scan halted with error: {}", e);
                        emit_log("error", &err_msg, &proxy_clone);
                        emit_event(
                            "scan_error",
                            &serde_json::json!({ "error": err_msg }).to_string(),
                            &proxy_clone,
                        );
                    }
                }
            });
        }
        "cancel_scan" => {
            emit_log("warn", "Cancellation requested by user...", proxy);
            if let Ok(st) = state.lock() {
                st.cancel_token.cancel();
            }
        }
        "get_file_preview" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default();
            let offset = msg.payload.get("source_offset").and_then(|v| v.as_u64()).unwrap_or(0);
            let size = msg.payload.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
            let ext = msg.payload.get("extension").and_then(|v| v.as_str()).unwrap_or_default();

            emit_log("info", &format!("Generating pre-recovery preview (ext: {}, offset: 0x{:X}, size: {} bytes)...", ext, offset, size), proxy);

            let preview = if let Ok(mut reader) = open_block_reader(source_id) {
                let read_limit = (size as usize).min(12 * 1024 * 1024).max(512);
                let mut buf = vec![0u8; read_limit];
                let _ = reader.read_at(offset, &mut buf);
                generate_preview(&buf, ext)
            } else {
                generate_preview(&[], ext)
            };

            let json = serde_json::to_string(&preview).unwrap_or_else(|_| "{}".to_string());
            emit_response(msg_id, &json, proxy);
        }
        "repair_file" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default();
            let offset = msg.payload.get("source_offset").and_then(|v| v.as_u64()).unwrap_or(0);
            let size = msg.payload.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
            let fname = msg.payload.get("filename").and_then(|v| v.as_str()).unwrap_or_default();

            emit_log("info", &format!("Running non-destructive image validation & repair on '{}'...", fname), proxy);

            let mut success = false;
            let mut fixes = Vec::new();
            let mut issues = Vec::new();

            if let Ok(mut reader) = open_block_reader(source_id) {
                let mut buf = vec![0u8; size.min(10 * 1024 * 1024) as usize];
                if reader.read_at(offset, &mut buf).is_ok() {
                    if let Ok(report) = repair_image(&buf, fname) {
                        success = report.success;
                        fixes = report.fixes_applied;
                        issues = report.issues_found;
                    }
                }
            }

            if success {
                emit_log("success", &format!("Repair analysis completed for '{}': {} fixes applied", fname, fixes.len()), proxy);
            } else {
                emit_log("warn", &format!("Repair analysis completed for '{}': {} issues found", fname, issues.len()), proxy);
            }

            let json = serde_json::json!({
                "success": success,
                "fixes_applied": fixes,
                "issues_found": issues,
            })
            .to_string();
            emit_response(msg_id, &json, proxy);
        }
        "recover_files" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let dest_path = msg.payload.get("destination_path").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let files_val = msg.payload.get("files").cloned().unwrap_or(serde_json::Value::Null);

            let files: Vec<RecoveredFile> = serde_json::from_value(files_val).unwrap_or_default();
            let cancel = CancellationToken::new();

            emit_log(
                "info",
                &format!("Starting extraction of {} files to safe target: '{}'", files.len(), dest_path),
                proxy,
            );

            let mut report_md = String::new();
            if let Ok(mut reader) = open_block_reader(&source_id) {
                if let Ok(report) = Scanner::recover_files(
                    &mut *reader,
                    &files,
                    Path::new(&dest_path),
                    &cancel,
                    |_f, _d, _t| {},
                ) {
                    emit_log(
                        "success",
                        &format!("Recovery completed: {} files written to '{}'", files.len(), dest_path),
                        proxy,
                    );
                    report_md = report.to_markdown();
                }
            }

            let json = serde_json::json!({
                "success": true,
                "report_markdown": report_md
            })
            .to_string();
            emit_response(msg_id, &json, proxy);
        }
        "open_output_folder" => {
            if let Some(path) = msg.payload.get("path").and_then(|v| v.as_str()) {
                emit_log("info", &format!("Opening output folder in OS file explorer: {}", path), proxy);
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("explorer").arg(path).spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(path).spawn();
            }
        }
        _ => {}
    }
}

fn emit_log(level: &str, message: &str, proxy: &EventLoopProxy<UserEvent>) {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    let payload = serde_json::json!({
        "timestamp": now,
        "level": level,
        "message": message,
    });
    emit_event("openre-log", &payload.to_string(), proxy);
}

fn emit_response(msg_id: Option<String>, json_data: &str, proxy: &EventLoopProxy<UserEvent>) {
    let encoded_msg_id = serde_json::to_string(&msg_id).unwrap_or_else(|_| "null".to_string());
    let encoded_data = serde_json::to_string(json_data).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        "try {{ window.dispatchEvent(new CustomEvent('openre-response', {{ detail: {{ msgId: {}, data: JSON.parse({}) }} }})); }} catch(err) {{ console.error('Failed to dispatch openre-response:', err); }}",
        encoded_msg_id,
        encoded_data
    );
    let _ = proxy.send_event(UserEvent::EvaluateScript(script));
}

fn emit_event(event_type: &str, json_payload: &str, proxy: &EventLoopProxy<UserEvent>) {
    let encoded_payload = serde_json::to_string(json_payload).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        "try {{ window.dispatchEvent(new CustomEvent('openre-event', {{ detail: {{ type: '{}', payload: JSON.parse({}) }} }})); }} catch(err) {{ console.error('Failed to dispatch openre-event:', err); }}",
        event_type,
        encoded_payload
    );
    let _ = proxy.send_event(UserEvent::EvaluateScript(script));
}
