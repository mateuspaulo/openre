#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use device_access::{enumerate_devices, open_block_reader, BlockReader};
use file_preview::{generate_preview, generate_thumbnail_data_url};
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
    current_source_id: String,
}

#[derive(serde::Serialize)]
struct PageItem {
    #[serde(flatten)]
    file: RecoveredFile,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail_url: Option<String>,
}

fn load_thumbnail_for_file(
    file: &RecoveredFile,
    source_id: &str,
    reader_cache: &mut std::collections::HashMap<String, Box<dyn BlockReader>>,
) -> Option<String> {
    if file.category != recovery_core::FileCategory::Image {
        return None;
    }

    let effective_size = if file.size_bytes > 0 { file.size_bytes as usize } else { 2 * 1024 * 1024 };
    let read_limit = effective_size.min(4 * 1024 * 1024).max(16 * 1024);

    let buf_opt = if let Some(ref orig_path) = file.original_path {
        if !orig_path.is_empty() && Path::new(orig_path).is_file() {
            if let Ok(mut f) = std::fs::File::open(orig_path) {
                use std::io::Read;
                let mut buf = vec![0u8; read_limit];
                let n = f.read(&mut buf).unwrap_or(0);
                buf.truncate(n);
                Some(buf)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let buf_opt = match buf_opt {
        Some(b) => Some(b),
        None => {
            if !source_id.is_empty() {
                if !reader_cache.contains_key(source_id) {
                    if let Ok(r) = open_block_reader(source_id) {
                        reader_cache.insert(source_id.to_string(), r);
                    }
                }
                if let Some(r) = reader_cache.get_mut(source_id) {
                    let mut buf = vec![0u8; read_limit];
                    let n = r.read_at(file.source_offset, &mut buf).unwrap_or(0);
                    buf.truncate(n);
                    Some(buf)
                } else {
                    None
                }
            } else {
                None
            }
        }
    };

    if let Some(buf) = buf_opt {
        generate_thumbnail_data_url(&buf, &file.extension)
    } else {
        None
    }
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
        current_source_id: String::new(),
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
                st.current_source_id = source_id.clone();
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
                        let total_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();
                        let mut cat_counts = serde_json::Map::new();
                        for f in &files {
                            let cat_str = f.category.as_str();
                            let c = cat_counts.entry(cat_str).or_insert(serde_json::Value::from(0));
                            if let Some(n) = c.as_i64() {
                                *c = serde_json::Value::from(n + 1);
                            }
                        }

                        let mut reader_cache: std::collections::HashMap<String, Box<dyn BlockReader>> = std::collections::HashMap::new();
                        let initial_page: Vec<PageItem> = files
                            .iter()
                            .take(20)
                            .map(|f| {
                                let thumb = load_thumbnail_for_file(f, &source_id, &mut reader_cache);
                                PageItem {
                                    file: f.clone(),
                                    thumbnail_url: thumb,
                                }
                            })
                            .collect();

                        if let Ok(mut st) = state_clone.lock() {
                            st.last_discovered = files;
                        }
                        emit_log(
                            "success",
                            &format!("Scan completed! {} recoverable files identified.", count),
                            &proxy_clone,
                        );
                        emit_event(
                            "scan_completed",
                            &serde_json::json!({
                                "total_files": count,
                                "total_bytes": total_bytes,
                                "category_counts": cat_counts,
                                "initial_page": initial_page,
                                "files": initial_page
                            }).to_string(),
                            &proxy_clone,
                        );
                    }
                    Err(RecoveryError::Cancelled) => {
                        let preserved_files = state_clone
                            .lock()
                            .map(|st| st.last_discovered.clone())
                            .unwrap_or_default();
                        let count = preserved_files.len();
                        let total_bytes: u64 = preserved_files.iter().map(|f| f.size_bytes).sum();
                        let mut cat_counts = serde_json::Map::new();
                        for f in &preserved_files {
                            let cat_str = f.category.as_str();
                            let c = cat_counts.entry(cat_str).or_insert(serde_json::Value::from(0));
                            if let Some(n) = c.as_i64() {
                                *c = serde_json::Value::from(n + 1);
                            }
                        }
                        let mut reader_cache: std::collections::HashMap<String, Box<dyn BlockReader>> = std::collections::HashMap::new();
                        let initial_page: Vec<PageItem> = preserved_files
                            .iter()
                            .take(20)
                            .map(|f| {
                                let thumb = load_thumbnail_for_file(f, &source_id, &mut reader_cache);
                                PageItem {
                                    file: f.clone(),
                                    thumbnail_url: thumb,
                                }
                            })
                            .collect();

                        emit_log("info", &format!("Scan stopped by user. Preserved {} files discovered so far.", count), &proxy_clone);
                        emit_event(
                            "scan_completed",
                            &serde_json::json!({
                                "total_files": count,
                                "total_bytes": total_bytes,
                                "category_counts": cat_counts,
                                "initial_page": initial_page,
                                "files": initial_page
                            }).to_string(),
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
            emit_log("warn", "Scan stop requested by user...", proxy);
            if let Ok(st) = state.lock() {
                st.cancel_token.cancel();
            }
        }
        "get_file_preview" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default();
            let offset = msg.payload.get("source_offset").and_then(|v| v.as_u64()).unwrap_or(0);
            let size = msg.payload.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
            let ext = msg.payload.get("extension").and_then(|v| v.as_str()).unwrap_or_default();
            let orig_path = msg.payload.get("original_path").and_then(|v| v.as_str()).unwrap_or_default();

            emit_log("info", &format!("Generating pre-recovery preview for '{}' (ext: {}, offset: 0x{:X}, size: {} bytes)...", if orig_path.is_empty() { source_id } else { orig_path }, ext, offset, size), proxy);

            let effective_size = if size > 0 { size as usize } else { 4 * 1024 * 1024 };
            let read_limit = effective_size.min(12 * 1024 * 1024).max(64 * 1024);

            let preview = if !orig_path.is_empty() && Path::new(orig_path).is_file() {
                // If original file exists on filesystem (e.g. from Quick Scan), read it directly!
                if let Ok(mut f) = std::fs::File::open(orig_path) {
                    use std::io::Read;
                    let mut buf = vec![0u8; read_limit];
                    let n = f.read(&mut buf).unwrap_or(0);
                    buf.truncate(n);
                    generate_preview(&buf, ext)
                } else if let Ok(mut reader) = open_block_reader(source_id) {
                    let mut buf = vec![0u8; read_limit];
                    let n = reader.read_at(offset, &mut buf).unwrap_or(0);
                    buf.truncate(n);
                    generate_preview(&buf, ext)
                } else {
                    generate_preview(&[], ext)
                }
            } else if let Ok(mut reader) = open_block_reader(source_id) {
                let mut buf = vec![0u8; read_limit];
                let n = reader.read_at(offset, &mut buf).unwrap_or(0);
                buf.truncate(n);
                generate_preview(&buf, ext)
            } else {
                generate_preview(&[], ext)
            };

            let json = serde_json::to_string(&preview).unwrap_or_else(|_| "{}".to_string());
            emit_response(msg_id, &json, proxy);
        }
        "get_thumbnails" => {
            let items = msg.payload.get("items").and_then(|v| v.as_array());
            let mut results = serde_json::Map::new();

            if let Some(items) = items {
                let mut reader_cache: std::collections::HashMap<String, Box<dyn BlockReader>> = std::collections::HashMap::new();

                for item in items {
                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                    if id.is_empty() {
                        continue;
                    }
                    let source_id = item.get("source_id").and_then(|v| v.as_str()).unwrap_or_default();
                    let offset = item.get("source_offset").and_then(|v| v.as_u64()).unwrap_or(0);
                    let size = item.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                    let ext = item.get("extension").and_then(|v| v.as_str()).unwrap_or_default();
                    let orig_path = item.get("original_path").and_then(|v| v.as_str()).unwrap_or_default();

                    let effective_size = if size > 0 { size as usize } else { 2 * 1024 * 1024 };
                    let read_limit = effective_size.min(4 * 1024 * 1024).max(16 * 1024);

                    let buf_opt = if !orig_path.is_empty() && Path::new(orig_path).is_file() {
                        if let Ok(mut f) = std::fs::File::open(orig_path) {
                            use std::io::Read;
                            let mut buf = vec![0u8; read_limit];
                            let n = f.read(&mut buf).unwrap_or(0);
                            buf.truncate(n);
                            Some(buf)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let buf_opt = match buf_opt {
                        Some(b) => Some(b),
                        None => {
                            if !source_id.is_empty() {
                                if !reader_cache.contains_key(source_id) {
                                    if let Ok(r) = open_block_reader(source_id) {
                                        reader_cache.insert(source_id.to_string(), r);
                                    }
                                }
                                if let Some(r) = reader_cache.get_mut(source_id) {
                                    let mut buf = vec![0u8; read_limit];
                                    let n = r.read_at(offset, &mut buf).unwrap_or(0);
                                    buf.truncate(n);
                                    Some(buf)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                    };

                    if let Some(buf) = buf_opt {
                        if let Some(thumb) = generate_thumbnail_data_url(&buf, ext) {
                            results.insert(id.to_string(), serde_json::Value::String(thumb));
                        }
                    }
                }
            }

            let resp = serde_json::json!({ "thumbnails": results });
            emit_response(msg_id, &resp.to_string(), proxy);
        }
        "get_files_page" => {
            let page = msg.payload.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let page_size = msg.payload.get("page_size").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let category_filter = msg.payload.get("category").and_then(|v| v.as_str()).unwrap_or("All");
            let search = msg.payload.get("search").and_then(|v| v.as_str()).unwrap_or("").trim().to_lowercase();
            let mut source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

            let page = page.max(1);
            let page_size = page_size.max(1);

            if let Ok(st) = state.lock() {
                if source_id.is_empty() {
                    source_id = st.current_source_id.clone();
                }

                let mut cat_counts = serde_json::Map::new();
                for f in &st.last_discovered {
                    let cat_str = f.category.as_str();
                    let c = cat_counts.entry(cat_str).or_insert(serde_json::Value::from(0));
                    if let Some(n) = c.as_i64() {
                        *c = serde_json::Value::from(n + 1);
                    }
                }

                let filtered: Vec<&RecoveredFile> = st.last_discovered.iter().filter(|f| {
                    let matches_cat = category_filter == "All" || f.category.as_str().eq_ignore_ascii_case(category_filter);
                    if !matches_cat {
                        return false;
                    }
                    if search.is_empty() {
                        return true;
                    }
                    f.filename.to_lowercase().contains(&search) || f.extension.to_lowercase().contains(&search)
                }).collect();

                let total_items = filtered.len();
                let total_pages = if total_items == 0 { 1 } else { (total_items + page_size - 1) / page_size };
                let safe_page = page.min(total_pages);
                let start_idx = (safe_page - 1) * page_size;

                let mut reader_cache: std::collections::HashMap<String, Box<dyn BlockReader>> = std::collections::HashMap::new();
                let page_items: Vec<PageItem> = filtered
                    .into_iter()
                    .skip(start_idx)
                    .take(page_size)
                    .map(|f| {
                        let thumb = load_thumbnail_for_file(f, &source_id, &mut reader_cache);
                        PageItem {
                            file: (*f).clone(),
                            thumbnail_url: thumb,
                        }
                    })
                    .collect();

                let resp = serde_json::json!({
                    "items": page_items,
                    "page": safe_page,
                    "page_size": page_size,
                    "total_items": total_items,
                    "total_pages": total_pages,
                    "total_discovered": st.last_discovered.len(),
                    "category_counts": cat_counts
                });
                emit_response(msg_id, &resp.to_string(), proxy);
            } else {
                let resp = serde_json::json!({
                    "items": [],
                    "page": 1,
                    "page_size": page_size,
                    "total_items": 0,
                    "total_pages": 1,
                    "total_discovered": 0,
                    "category_counts": {}
                });
                emit_response(msg_id, &resp.to_string(), proxy);
            }
        }
        "repair_file" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default();
            let offset = msg.payload.get("source_offset").and_then(|v| v.as_u64()).unwrap_or(0);
            let size = msg.payload.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
            let fname = msg.payload.get("filename").and_then(|v| v.as_str()).unwrap_or_default();
            let orig_path = msg.payload.get("original_path").and_then(|v| v.as_str()).unwrap_or_default();

            emit_log("info", &format!("Initiating single-file extraction & restoration for '{}'...", fname), proxy);

            let mut success = false;
            let mut fixes = Vec::new();
            let mut issues = Vec::new();
            let mut saved = false;
            let mut saved_path_str: Option<String> = None;
            let mut user_cancelled = false;

            let effective_size = if size > 0 { size as usize } else { 4 * 1024 * 1024 };
            let read_limit = effective_size.min(100 * 1024 * 1024); // Up to 100MB per single file restore

            let buf_opt = if !orig_path.is_empty() && Path::new(orig_path).is_file() {
                if let Ok(mut f) = std::fs::File::open(orig_path) {
                    use std::io::Read;
                    let mut b = vec![0u8; read_limit];
                    let n = f.read(&mut b).unwrap_or(0);
                    b.truncate(n);
                    Some(b)
                } else {
                    None
                }
            } else if let Ok(mut reader) = open_block_reader(source_id) {
                let mut b = vec![0u8; read_limit];
                let n = reader.read_at(offset, &mut b).unwrap_or(0);
                b.truncate(n);
                Some(b)
            } else {
                None
            };

            if let Some(buf) = buf_opt {
                let clean_name = Path::new(fname)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "recovered_file".to_string());
                let ext = Path::new(&clean_name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let is_image = ext == "jpg" || ext == "jpeg" || ext == "png";

                let data_to_save = if is_image {
                    if let Ok(report) = repair_image(&buf, fname) {
                        success = report.success;
                        fixes = report.fixes_applied;
                        issues = report.issues_found;
                        if !report.repaired_data.is_empty() {
                            report.repaired_data
                        } else {
                            buf
                        }
                    } else {
                        buf
                    }
                } else {
                    success = true;
                    fixes.push(format!("Extracted {} bytes directly from source storage", buf.len()));
                    buf
                };

                let default_name = if is_image && !fixes.is_empty() {
                    format!("repaired_{}", clean_name)
                } else {
                    format!("recovered_{}", clean_name)
                };

                let mut dialog = rfd::FileDialog::new()
                    .set_title("Save File Copy As / Salvar Cópia Como")
                    .set_file_name(&default_name);

                if !ext.is_empty() {
                    dialog = dialog.add_filter(&format!("{} File", ext.to_uppercase()), &[&ext]);
                }

                if let Some(dest_path) = dialog.save_file() {
                    if std::fs::write(&dest_path, &data_to_save).is_ok() {
                        saved = true;
                        let p_str = dest_path.to_string_lossy().to_string();
                        emit_log("success", &format!("File copy saved successfully to: '{}'", p_str), proxy);
                        saved_path_str = Some(p_str);
                    } else {
                        emit_log("error", &format!("Failed to write file to: '{}'", dest_path.display()), proxy);
                    }
                } else {
                    user_cancelled = true;
                    emit_log("info", "Save destination selection cancelled by user.", proxy);
                }
            }

            if saved {
                emit_log("success", &format!("File copy for '{}' saved successfully.", fname), proxy);
            } else if user_cancelled {
                emit_log("info", &format!("Save destination for '{}' cancelled by user.", fname), proxy);
            } else if success {
                emit_log("info", &format!("Restoration analysis ready for '{}'.", fname), proxy);
            } else {
                emit_log("warn", &format!("Restoration warning for '{}': issues encountered.", fname), proxy);
            }

            let json = serde_json::json!({
                "success": success,
                "saved": saved,
                "saved_path": saved_path_str,
                "user_cancelled": user_cancelled,
                "fixes_applied": fixes,
                "issues_found": issues,
            })
            .to_string();
            emit_response(msg_id, &json, proxy);
        }
        "recover_files" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let dest_path = msg.payload.get("destination_path").and_then(|v| v.as_str()).unwrap_or_default().to_string();

            let select_all = msg.payload.get("select_all").and_then(|v| v.as_bool()).unwrap_or(false);
            let excluded_ids: std::collections::HashSet<String> = msg.payload.get("excluded_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let included_ids: std::collections::HashSet<String> = msg.payload.get("included_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let category_filter = msg.payload.get("category").and_then(|v| v.as_str()).unwrap_or("All");
            let search_query = msg.payload.get("search").and_then(|v| v.as_str()).unwrap_or("").trim().to_lowercase();

            let files_val = msg.payload.get("files").cloned().unwrap_or(serde_json::Value::Null);
            let direct_files: Vec<RecoveredFile> = serde_json::from_value(files_val).unwrap_or_default();

            let files: Vec<RecoveredFile> = if !direct_files.is_empty() {
                direct_files
            } else if let Ok(st) = state.lock() {
                if select_all {
                    st.last_discovered.iter().filter(|f| {
                        let matches_cat = category_filter == "All" || f.category.as_str().eq_ignore_ascii_case(category_filter);
                        if !matches_cat {
                            return false;
                        }
                        if !search_query.is_empty() && !f.filename.to_lowercase().contains(&search_query) && !f.extension.to_lowercase().contains(&search_query) {
                            return false;
                        }
                        !excluded_ids.contains(&f.id)
                    }).cloned().collect()
                } else {
                    st.last_discovered.iter().filter(|f| included_ids.contains(&f.id)).cloned().collect()
                }
            } else {
                Vec::new()
            };

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
                {
                    let p = std::path::Path::new(path);
                    if p.is_file() {
                        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path)).spawn();
                    } else {
                        let _ = std::process::Command::new("explorer").arg(path).spawn();
                    }
                }
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
