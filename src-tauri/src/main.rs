#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use device_access::{enumerate_devices, open_block_reader, BlockReader};
use file_preview::{generate_preview, generate_thumbnail_data_url};
use image_repair::repair_image;
use recovery_core::{CancellationToken, RecoveredFile, RecoveryError};
use scanner::Scanner;
use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
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
    thumbnail_cache: Arc<Mutex<std::collections::HashMap<String, Option<String>>>>,
    media_port: u16,
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
    thumbnail_cache: &Arc<Mutex<std::collections::HashMap<String, Option<String>>>>,
) -> Option<String> {
    if file.category != recovery_core::FileCategory::Image {
        return None;
    }

    let cache_key = format!("{}_{}", file.id, file.source_offset);
    if let Ok(cache) = thumbnail_cache.lock() {
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    let effective_size = if file.size_bytes > 0 { file.size_bytes as usize } else { 256 * 1024 };
    let read_limit = effective_size.min(384 * 1024).max(16 * 1024);

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

    let thumb = if let Some(buf) = buf_opt {
        generate_thumbnail_data_url(&buf, &file.extension)
    } else {
        None
    };

    if let Ok(mut cache) = thumbnail_cache.lock() {
        cache.insert(cache_key, thumb.clone());
    }

    thumb
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes().peekable();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next().unwrap_or(b'0');
            let h2 = chars.next().unwrap_or(b'0');
            let hex_str = [h1, h2];
            if let Ok(s) = std::str::from_utf8(&hex_str) {
                if let Ok(byte_val) = u8::from_str_radix(s, 16) {
                    bytes.push(byte_val);
                    continue;
                }
            }
            bytes.push(b'%');
            bytes.push(h1);
            bytes.push(h2);
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn get_mime_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" | "aac" => "audio/mp4",
        "opus" => "audio/opus",
        "wma" => "audio/x-ms-wma",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn parse_range_header(header_val: &str, total_len: u64) -> Option<(u64, u64)> {
    let s = header_val.trim();
    let prefix = "bytes=";
    if !s.to_lowercase().starts_with(prefix) {
        return None;
    }
    let range_str = &s[prefix.len()..];
    let mut parts = range_str.split('-');
    let start_str = parts.next()?.trim();
    let end_str = parts.next()?.trim();

    if start_str.is_empty() {
        if total_len == 0 {
            return None;
        }
        let suffix_len: u64 = end_str.parse().ok()?;
        let start = total_len.saturating_sub(suffix_len);
        let end = total_len.saturating_sub(1);
        Some((start, end))
    } else {
        let start: u64 = start_str.parse().ok()?;
        if total_len == 0 {
            return None;
        }
        let end: u64 = if end_str.is_empty() {
            total_len.saturating_sub(1)
        } else {
            let parsed_end: u64 = end_str.parse().ok()?;
            parsed_end.min(total_len.saturating_sub(1))
        };
        if start <= end && start < total_len {
            Some((start, end))
        } else {
            None
        }
    }
}

fn handle_media_connection(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req_str.lines();
    let req_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let mut parts = req_line.split_whitespace();
    let method = match parts.next() {
        Some(m) => m,
        None => return,
    };
    let uri = match parts.next() {
        Some(u) => u,
        None => return,
    };

    if method == "OPTIONS" {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, HEAD, OPTIONS\r\nAccess-Control-Allow-Headers: Range, Content-Type\r\nAccess-Control-Max-Age: 86400\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if method != "GET" && method != "HEAD" {
        let resp = "HTTP/1.1 405 Method Not Allowed\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    let mut range_header = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("range") {
                range_header = Some(v.trim().to_string());
            }
        }
    }

    let (path_part, query_part) = match uri.split_once('?') {
        Some((p, q)) => (p, q),
        None => (uri, ""),
    };

    let mut query_params: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for pair in query_part.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            query_params.insert(k.to_string(), percent_decode(v));
        }
    }

    if path_part == "/stream" {
        let file_path = match query_params.get("path") {
            Some(p) if !p.is_empty() => p,
            _ => {
                let resp = "HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        };

        let mut file = match std::fs::File::open(file_path) {
            Ok(f) => f,
            Err(_) => {
                let resp = "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        };

        let total_size = match file.metadata() {
            Ok(m) => m.len(),
            Err(_) => {
                let resp = "HTTP/1.1 500 Internal Error\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        };

        let ext = Path::new(file_path).extension().and_then(|e| e.to_str()).unwrap_or("");
        let mime = get_mime_type(ext);

        if let Some(ref range_str) = range_header {
            if let Some((start, end)) = parse_range_header(range_str, total_size) {
                let content_len = end - start + 1;
                let header = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Type: {}\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Range\r\nConnection: close\r\n\r\n",
                    mime, start, end, total_size, content_len
                );
                if stream.write_all(header.as_bytes()).is_err() {
                    return;
                }
                if method == "GET" {
                    if file.seek(SeekFrom::Start(start)).is_ok() {
                        let mut remaining = content_len;
                        let mut chunk = vec![0u8; 64 * 1024];
                        while remaining > 0 {
                            let to_read = (remaining as usize).min(chunk.len());
                            let n = match file.read(&mut chunk[..to_read]) {
                                Ok(n) if n > 0 => n,
                                _ => break,
                            };
                            if stream.write_all(&chunk[..n]).is_err() {
                                break;
                            }
                            remaining -= n as u64;
                        }
                    }
                }
                return;
            } else {
                let resp = format!(
                    "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                    total_size
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        }

        // Full content response (no Range header)
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Range\r\nConnection: close\r\n\r\n",
            mime, total_size
        );
        if stream.write_all(header.as_bytes()).is_err() {
            return;
        }
        if method == "GET" {
            let mut chunk = vec![0u8; 64 * 1024];
            loop {
                let n = match file.read(&mut chunk) {
                    Ok(n) if n > 0 => n,
                    _ => break,
                };
                if stream.write_all(&chunk[..n]).is_err() {
                    break;
                }
            }
        }
    } else if path_part == "/carved" {
        let source_id = query_params.get("source_id").cloned().unwrap_or_default();
        let offset: u64 = query_params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
        let size: u64 = query_params.get("size").and_then(|v| v.parse().ok()).unwrap_or(0);
        let ext = query_params.get("ext").cloned().unwrap_or_default();

        if source_id.is_empty() || size == 0 {
            let resp = "HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
            return;
        }

        let mut reader = match open_block_reader(&source_id) {
            Ok(r) => r,
            Err(_) => {
                let resp = "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        };

        let mime = get_mime_type(&ext);
        let total_size = size;

        if let Some(ref range_str) = range_header {
            if let Some((start, end)) = parse_range_header(range_str, total_size) {
                let content_len = end - start + 1;
                let header = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Type: {}\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Range\r\nConnection: close\r\n\r\n",
                    mime, start, end, total_size, content_len
                );
                if stream.write_all(header.as_bytes()).is_err() {
                    return;
                }
                if method == "GET" {
                    let mut cur_offset = offset + start;
                    let mut remaining = content_len;
                    let mut chunk = vec![0u8; 64 * 1024];
                    while remaining > 0 {
                        let to_read = (remaining as usize).min(chunk.len());
                        let n = match reader.read_at(cur_offset, &mut chunk[..to_read]) {
                            Ok(n) if n > 0 => n,
                            _ => break,
                        };
                        if stream.write_all(&chunk[..n]).is_err() {
                            break;
                        }
                        cur_offset += n as u64;
                        remaining -= n as u64;
                    }
                }
                return;
            } else {
                let resp = format!(
                    "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                    total_size
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        }

        // Full content response (no Range header)
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Range\r\nConnection: close\r\n\r\n",
            mime, total_size
        );
        if stream.write_all(header.as_bytes()).is_err() {
            return;
        }
        if method == "GET" {
            let mut cur_offset = offset;
            let mut remaining = total_size;
            let mut chunk = vec![0u8; 64 * 1024];
            while remaining > 0 {
                let to_read = (remaining as usize).min(chunk.len());
                let n = match reader.read_at(cur_offset, &mut chunk[..to_read]) {
                    Ok(n) if n > 0 => n,
                    _ => break,
                };
                if stream.write_all(&chunk[..n]).is_err() {
                    break;
                }
                cur_offset += n as u64;
                remaining -= n as u64;
            }
        }
    } else {
        let resp = "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes());
    }
}

fn start_media_server() -> Option<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                thread::spawn(move || {
                    handle_media_connection(stream);
                });
            }
        }
    });
    Some(port)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--allow-file-access-from-files",
    );

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("OpenRe — Professional File Recovery")
        .with_inner_size(LogicalSize::new(1240.0, 840.0))
        .with_min_inner_size(LogicalSize::new(960.0, 640.0))
        .build(&event_loop)?;

    let media_port = start_media_server().unwrap_or(0);
    emit_log("info", &format!("Internal loopback media streaming service active on 127.0.0.1:{}", media_port), &proxy);

    let app_state = Arc::new(Mutex::new(AppState {
        cancel_token: CancellationToken::new(),
        last_discovered: Vec::new(),
        current_source_id: String::new(),
        thumbnail_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        media_port,
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
    let media_port = if let Ok(st) = state.lock() { st.media_port } else { 0 };

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
                if let Ok(mut tc) = st.thumbnail_cache.lock() {
                    tc.clear();
                }
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

                        let thumb_cache = state_clone
                            .lock()
                            .map(|st| Arc::clone(&st.thumbnail_cache))
                            .unwrap_or_else(|_| Arc::new(Mutex::new(std::collections::HashMap::new())));

                        let mut reader_cache: std::collections::HashMap<String, Box<dyn BlockReader>> = std::collections::HashMap::new();
                        let initial_page: Vec<PageItem> = files
                            .iter()
                            .take(20)
                            .map(|f| {
                                let thumb = load_thumbnail_for_file(f, &source_id, &mut reader_cache, &thumb_cache);
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
                        let thumb_cache = state_clone
                            .lock()
                            .map(|st| Arc::clone(&st.thumbnail_cache))
                            .unwrap_or_else(|_| Arc::new(Mutex::new(std::collections::HashMap::new())));

                        let mut reader_cache: std::collections::HashMap<String, Box<dyn BlockReader>> = std::collections::HashMap::new();
                        let initial_page: Vec<PageItem> = preserved_files
                            .iter()
                            .take(20)
                            .map(|f| {
                                let thumb = load_thumbnail_for_file(f, &source_id, &mut reader_cache, &thumb_cache);
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

            let effective_size = if size > 0 { size as usize } else { 2 * 1024 * 1024 };
            let read_limit = effective_size.min(4 * 1024 * 1024).max(64 * 1024);

            let mut preview = if !orig_path.is_empty() && Path::new(orig_path).is_file() {
                if let Ok(mut f) = std::fs::File::open(orig_path) {
                    let mut buf = vec![0u8; read_limit];
                    let n = f.read(&mut buf).unwrap_or(0);
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

            // Enhance audio and video previews with instant, high-performance loopback HTTP streaming
            let cat = recovery_core::FileCategory::from_extension(ext);
            if cat == recovery_core::FileCategory::Audio || cat == recovery_core::FileCategory::Video {
                if media_port > 0 {
                    if !orig_path.is_empty() && Path::new(orig_path).is_file() {
                        preview.media_data_url = Some(format!(
                            "http://127.0.0.1:{}/stream?path={}",
                            media_port,
                            percent_encode(orig_path)
                        ));
                    } else if !source_id.is_empty() && size > 0 {
                        preview.media_data_url = Some(format!(
                            "http://127.0.0.1:{}/carved?source_id={}&offset={}&size={}&ext={}",
                            media_port,
                            percent_encode(source_id),
                            offset,
                            size,
                            percent_encode(ext)
                        ));
                    }
                }
            }

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
            let page_size = page_size.clamp(1, 500);

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

                let thumb_cache = Arc::clone(&st.thumbnail_cache);
                let mut reader_cache: std::collections::HashMap<String, Box<dyn BlockReader>> = std::collections::HashMap::new();
                let page_items: Vec<PageItem> = filtered
                    .into_iter()
                    .skip(start_idx)
                    .take(page_size)
                    .map(|f| {
                        let thumb = load_thumbnail_for_file(f, &source_id, &mut reader_cache, &thumb_cache);
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
            let proxy_clone = proxy.clone();

            thread::spawn(move || {
                emit_log(
                    "info",
                    &format!("Starting extraction of {} files to safe target: '{}'", files.len(), dest_path),
                    &proxy_clone,
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
                            &proxy_clone,
                        );
                        report_md = report.to_markdown();
                    }
                }

                let json = serde_json::json!({
                    "success": true,
                    "report_markdown": report_md
                })
                .to_string();
                emit_response(msg_id, &json, &proxy_clone);
            });
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
