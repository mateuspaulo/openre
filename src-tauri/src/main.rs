#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use device_access::{enumerate_devices, BlockReader, FileBlockReader};
use file_preview::generate_preview;
use image_repair::repair_image;
use recovery_core::{CancellationToken, RecoveredFile};
use scanner::Scanner;
use serde::Deserialize;
use std::path::{Path, PathBuf};
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
        .with_inner_size(LogicalSize::new(1240.0, 820.0))
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
            emit_response(msg_id, &json, proxy);
        }
        "select_folder" => {
            let folder = rfd::FileDialog::new().pick_folder();
            let path_str = folder.map(|p| p.to_string_lossy().to_string());
            let json = serde_json::to_string(&path_str).unwrap_or_else(|_| "null".to_string());
            emit_response(msg_id, &json, proxy);
        }
        "select_image_file" => {
            let file = rfd::FileDialog::new()
                .add_filter("Raw Disk Images", &["img", "dd", "raw", "bin"])
                .pick_file();
            let path_str = file.map(|p| p.to_string_lossy().to_string());
            let json = serde_json::to_string(&path_str).unwrap_or_else(|_| "null".to_string());
            emit_response(msg_id, &json, proxy);
        }
        "start_scan" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let mode = msg.payload.get("mode").and_then(|v| v.as_str()).unwrap_or("quick").to_string();

            let cancel = CancellationToken::new();
            if let Ok(mut st) = state.lock() {
                st.cancel_token = cancel.clone();
            }

            let proxy_clone = proxy.clone();
            let state_clone = Arc::clone(state);

            thread::spawn(move || {
                let path = PathBuf::from(&source_id);
                if let Ok(mut reader) = FileBlockReader::open(&path) {
                    let proxy_progress = proxy_clone.clone();
                    let files_res = if mode == "quick" {
                        Scanner::quick_scan(&mut reader, &cancel, move |p| {
                            emit_event("scan_progress", &serde_json::to_string(&p).unwrap(), &proxy_progress);
                        })
                    } else {
                        Scanner::deep_scan(&mut reader, &cancel, move |p| {
                            emit_event("scan_progress", &serde_json::to_string(&p).unwrap(), &proxy_progress);
                        })
                    };

                    if let Ok(files) = files_res {
                        if let Ok(mut st) = state_clone.lock() {
                            st.last_discovered = files.clone();
                        }
                        emit_event(
                            "scan_completed",
                            &serde_json::json!({ "files": files }).to_string(),
                            &proxy_clone,
                        );
                    }
                }
            });
        }
        "cancel_scan" => {
            if let Ok(st) = state.lock() {
                st.cancel_token.cancel();
            }
        }
        "get_file_preview" => {
            let source_id = msg.payload.get("source_id").and_then(|v| v.as_str()).unwrap_or_default();
            let offset = msg.payload.get("source_offset").and_then(|v| v.as_u64()).unwrap_or(0);
            let size = msg.payload.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
            let ext = msg.payload.get("extension").and_then(|v| v.as_str()).unwrap_or_default();

            let preview = if let Ok(mut reader) = FileBlockReader::open(Path::new(source_id)) {
                let mut buf = vec![0u8; size.min(1024 * 1024) as usize];
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

            let mut success = false;
            let mut fixes = Vec::new();
            let mut issues = Vec::new();

            if let Ok(mut reader) = FileBlockReader::open(Path::new(source_id)) {
                let mut buf = vec![0u8; size.min(10 * 1024 * 1024) as usize];
                if reader.read_at(offset, &mut buf).is_ok() {
                    if let Ok(report) = repair_image(&buf, fname) {
                        success = report.success;
                        fixes = report.fixes_applied;
                        issues = report.issues_found;
                    }
                }
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

            let mut report_md = String::new();
            if let Ok(mut reader) = FileBlockReader::open(Path::new(&source_id)) {
                if let Ok(report) = Scanner::recover_files(
                    &mut reader,
                    &files,
                    Path::new(&dest_path),
                    &cancel,
                    |_f, _d, _t| {},
                ) {
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
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("explorer").arg(path).spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(path).spawn();
            }
        }
        _ => {}
    }
}

fn emit_response(msg_id: Option<String>, json_data: &str, proxy: &EventLoopProxy<UserEvent>) {
    let script = format!(
        "window.dispatchEvent(new CustomEvent('openre-response', {{ detail: {{ msgId: {}, data: {} }} }}));",
        serde_json::to_string(&msg_id).unwrap(),
        json_data
    );
    let _ = proxy.send_event(UserEvent::EvaluateScript(script));
}

fn emit_event(event_type: &str, json_payload: &str, proxy: &EventLoopProxy<UserEvent>) {
    let script = format!(
        "window.dispatchEvent(new CustomEvent('openre-event', {{ detail: {{ type: '{}', payload: {} }} }}));",
        event_type, json_payload
    );
    let _ = proxy.send_event(UserEvent::EvaluateScript(script));
}
