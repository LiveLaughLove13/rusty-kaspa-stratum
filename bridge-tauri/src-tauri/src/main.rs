//! **RKStratum Bridge Desktop** — optional GUI to configure the bridge, then **Start** (same `runner::run` as
//! `stratum-bridge`). Launch with extra CLI arguments to skip the form and start immediately (scripting).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

use clap::Parser;
use kaspa_alloc::init_allocator_with_default_settings;
use kaspa_stratum_bridge::cli::Cli;
use kaspa_stratum_bridge::{default_dashboard_iframe_url, request_bridge_shutdown, run};
use serde::Serialize;
use tauri::{CustomMenuItem, Manager, Menu, Submenu};

struct RunningBridge {
    join: std::thread::JoinHandle<()>,
    #[allow(dead_code)]
    cli: Cli,
}

#[derive(Default)]
struct AppState {
    bridge: Mutex<Option<RunningBridge>>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartBridgeDto {
    config: Option<String>,
    #[serde(default)]
    testnet: bool,
    node_mode: Option<String>,
    appdir: Option<String>,
    coinbase_tag_suffix: Option<String>,
    kaspad_address: Option<String>,
    block_wait_time_ms: Option<u64>,
    print_stats: Option<bool>,
    log_to_file: Option<bool>,
    health_check_port: Option<String>,
    web_dashboard_port: Option<String>,
    var_diff: Option<bool>,
    shares_per_min: Option<u32>,
    var_diff_stats: Option<bool>,
    extranonce_size: Option<u8>,
    pow2_clamp: Option<bool>,
    approximate_geo_lookup: Option<bool>,
    stratum_port: Option<String>,
    min_share_diff: Option<u32>,
    prom_port: Option<String>,
    #[serde(default)]
    instances: Vec<String>,
    instance_log_to_file: Option<bool>,
    instance_var_diff: Option<bool>,
    instance_shares_per_min: Option<u32>,
    instance_var_diff_stats: Option<bool>,
    instance_pow2_clamp: Option<bool>,
    #[serde(default)]
    #[cfg_attr(not(feature = "rkstratum_cpu_miner"), allow(dead_code))]
    internal_cpu_miner: bool,
    #[cfg_attr(not(feature = "rkstratum_cpu_miner"), allow(dead_code))]
    internal_cpu_miner_address: Option<String>,
    #[cfg_attr(not(feature = "rkstratum_cpu_miner"), allow(dead_code))]
    internal_cpu_miner_threads: Option<usize>,
    #[cfg_attr(not(feature = "rkstratum_cpu_miner"), allow(dead_code))]
    internal_cpu_miner_throttle_ms: Option<u64>,
    #[cfg_attr(not(feature = "rkstratum_cpu_miner"), allow(dead_code))]
    internal_cpu_miner_template_poll_ms: Option<u64>,
    #[serde(default)]
    kaspad_extra_args: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GuiDefaults {
    config_path: Option<String>,
    exe_directory: Option<String>,
    suggested_appdir: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogTailRequest {
    cursor: Option<u64>,
    max_bytes: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogTailResponse {
    cursor: u64,
    text: String,
    path: Option<String>,
}

fn bridge_logs_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join("kaspa-stratum-bridge").join("logs");
        }
        if let Some(home) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(home).join("kaspa-stratum-bridge").join("logs");
        }
        PathBuf::from(".").join("kaspa-stratum-bridge").join("logs")
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".kaspa-stratum-bridge").join("logs");
        }
        PathBuf::from(".").join(".kaspa-stratum-bridge").join("logs")
    }
}

fn newest_bridge_log_file() -> Option<PathBuf> {
    let dir = bridge_logs_dir();
    let entries = fs::read_dir(dir).ok()?;
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("RKStratum_") || !name.ends_with(".log") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        match &newest {
            Some((best, _)) if *best >= modified => {}
            _ => newest = Some((modified, path)),
        }
    }
    newest.map(|(_, p)| p)
}

fn spawn_running_bridge(cli: Cli) -> Result<RunningBridge, String> {
    std::env::set_var("RKSTRATUM_BRIDGE_EMBEDDED", "1");
    let cli_thread = cli.clone();
    let join = std::thread::Builder::new()
        .name("kaspa-stratum-bridge".into())
        .spawn(move || {
            init_allocator_with_default_settings();
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime");
            if let Err(e) = rt.block_on(run(cli_thread)) {
                eprintln!("stratum bridge: {e:#}");
            }
        })
        .map_err(|e| format!("failed to spawn bridge thread: {e}"))?;
    Ok(RunningBridge { join, cli })
}

fn cli_from_start_dto(dto: StartBridgeDto) -> Result<Cli, String> {
    let argv0 = std::env::args().next().unwrap_or_else(|| "rkstratum-bridge-desktop".into());
    let mut args = vec![argv0];
    if let Some(c) = dto.config {
        let c = c.trim();
        if !c.is_empty() {
            args.push("--config".into());
            args.push(c.to_string());
        }
    }
    if dto.testnet {
        args.push("--testnet".into());
    }
    if let Some(ref nm) = dto.node_mode {
        let nm = nm.trim();
        if !nm.is_empty() {
            args.push("--node-mode".into());
            args.push(nm.to_string());
        }
    }
    if let Some(a) = dto.appdir {
        let a = a.trim();
        if !a.is_empty() {
            args.push("--appdir".into());
            args.push(a.to_string());
        }
    }
    if let Some(s) = dto.coinbase_tag_suffix {
        let s = s.trim();
        if !s.is_empty() {
            args.push("--coinbase-tag-suffix".into());
            args.push(s.to_string());
        }
    }
    if let Some(addr) = dto.kaspad_address {
        let addr = addr.trim();
        if !addr.is_empty() {
            args.push("--kaspad-address".into());
            args.push(addr.to_string());
        }
    }
    if let Some(ms) = dto.block_wait_time_ms {
        args.push("--block-wait-time".into());
        args.push(ms.to_string());
    }
    if let Some(v) = dto.print_stats {
        args.push("--print-stats".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(v) = dto.log_to_file {
        args.push("--log-to-file".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(port) = dto.health_check_port {
        let port = port.trim();
        if !port.is_empty() {
            args.push("--health-check-port".into());
            args.push(port.to_string());
        }
    }
    if let Some(port) = dto.web_dashboard_port {
        let port = port.trim();
        if !port.is_empty() {
            args.push("--web-dashboard-port".into());
            args.push(port.to_string());
        }
    }
    if let Some(v) = dto.var_diff {
        args.push("--var-diff".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(v) = dto.shares_per_min {
        args.push("--shares-per-min".into());
        args.push(v.to_string());
    }
    if let Some(v) = dto.var_diff_stats {
        args.push("--var-diff-stats".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(v) = dto.extranonce_size {
        args.push("--extranonce-size".into());
        args.push(v.to_string());
    }
    if let Some(v) = dto.pow2_clamp {
        args.push("--pow2-clamp".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(v) = dto.approximate_geo_lookup {
        args.push("--approximate-geo-lookup".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(port) = dto.stratum_port {
        let port = port.trim();
        if !port.is_empty() {
            args.push("--stratum-port".into());
            args.push(port.to_string());
        }
    }
    if let Some(d) = dto.min_share_diff {
        args.push("--min-share-diff".into());
        args.push(d.to_string());
    }
    if let Some(port) = dto.prom_port {
        let port = port.trim();
        if !port.is_empty() {
            args.push("--prom-port".into());
            args.push(port.to_string());
        }
    }
    for spec in dto.instances {
        let spec = spec.trim();
        if spec.is_empty() {
            continue;
        }
        args.push("--instance".into());
        args.push(spec.to_string());
    }
    if let Some(v) = dto.instance_log_to_file {
        args.push("--instance-log-to-file".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(v) = dto.instance_var_diff {
        args.push("--instance-var-diff".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(v) = dto.instance_shares_per_min {
        args.push("--instance-shares-per-min".into());
        args.push(v.to_string());
    }
    if let Some(v) = dto.instance_var_diff_stats {
        args.push("--instance-var-diff-stats".into());
        args.push(if v { "true" } else { "false" }.into());
    }
    if let Some(v) = dto.instance_pow2_clamp {
        args.push("--instance-pow2-clamp".into());
        args.push(if v { "true" } else { "false" }.into());
    }

    #[cfg(feature = "rkstratum_cpu_miner")]
    {
        if dto.internal_cpu_miner {
            args.push("--internal-cpu-miner".into());
        }
        if let Some(s) = dto.internal_cpu_miner_address {
            let s = s.trim();
            if !s.is_empty() {
                args.push("--internal-cpu-miner-address".into());
                args.push(s.to_string());
            }
        }
        if let Some(n) = dto.internal_cpu_miner_threads {
            args.push("--internal-cpu-miner-threads".into());
            args.push(n.to_string());
        }
        if let Some(ms) = dto.internal_cpu_miner_throttle_ms {
            args.push("--internal-cpu-miner-throttle-ms".into());
            args.push(ms.to_string());
        }
        if let Some(ms) = dto.internal_cpu_miner_template_poll_ms {
            args.push("--internal-cpu-miner-template-poll-ms".into());
            args.push(ms.to_string());
        }
    }

    if !dto.kaspad_extra_args.is_empty() {
        args.push("--".into());
        args.extend(dto.kaspad_extra_args);
    }
    Cli::try_parse_from(args).map_err(|e| e.to_string())
}

#[tauri::command]
fn is_cli_mode() -> bool {
    std::env::args().nth(1).is_some()
}

#[tauri::command]
fn cpu_miner_feature_enabled() -> bool {
    cfg!(feature = "rkstratum_cpu_miner")
}

#[tauri::command]
fn gui_defaults() -> GuiDefaults {
    let exe = std::env::current_exe().ok();
    let exe_directory = exe.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().into_owned());
    let beside = exe.as_ref().and_then(|p| p.parent()).map(|d| d.join("config.yaml")).filter(|p| p.is_file());
    let config_path = beside.as_ref().map(|p| p.to_string_lossy().into_owned());
    let suggested_appdir = exe.as_ref().and_then(|p| p.parent()).map(|d| d.join("kaspa-data").to_string_lossy().into_owned());
    GuiDefaults { config_path, exe_directory, suggested_appdir }
}

#[tauri::command]
fn start_bridge(state: tauri::State<AppState>, dto: StartBridgeDto) -> Result<String, String> {
    let mut g = state.bridge.lock().map_err(|_| "bridge state lock poisoned".to_string())?;
    if g.is_some() {
        return Err("Bridge is already running. Use Bridge → Stop bridge first.".into());
    }
    let cli = cli_from_start_dto(dto)?;
    let url = default_dashboard_iframe_url(&cli);
    let running = spawn_running_bridge(cli)?;
    *g = Some(running);
    Ok(url)
}

#[tauri::command]
fn stop_bridge(state: tauri::State<AppState>) -> Result<(), String> {
    let mut g = state.bridge.lock().map_err(|_| "bridge state lock poisoned".to_string())?;
    let Some(running) = g.take() else {
        return Err("Bridge is not running.".into());
    };
    request_bridge_shutdown();
    running.join.join().map_err(|_| "Bridge thread panicked while stopping.".to_string())?;
    Ok(())
}

#[tauri::command]
fn bridge_is_running(state: tauri::State<AppState>) -> bool {
    state.bridge.lock().map(|g| g.is_some()).unwrap_or(false)
}

#[tauri::command]
fn dashboard_default_url(state: tauri::State<AppState>) -> Result<String, String> {
    let g = state.bridge.lock().map_err(|_| "lock poisoned".to_string())?;
    let Some(r) = g.as_ref() else {
        return Err("Bridge is not running.".into());
    };
    Ok(default_dashboard_iframe_url(&r.cli))
}

#[tauri::command]
fn bridge_log_tail(req: LogTailRequest) -> Result<LogTailResponse, String> {
    let Some(path) = newest_bridge_log_file() else {
        return Ok(LogTailResponse { cursor: 0, text: String::new(), path: None });
    };
    let mut file = fs::File::open(&path).map_err(|e| format!("open log failed: {e}"))?;
    let len = file.metadata().map_err(|e| format!("log metadata failed: {e}"))?.len();
    let mut cursor = req.cursor.unwrap_or(0);
    if cursor > len {
        cursor = 0;
    }
    file.seek(SeekFrom::Start(cursor)).map_err(|e| format!("seek log failed: {e}"))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|e| format!("read log failed: {e}"))?;
    let max_bytes = req.max_bytes.unwrap_or(32 * 1024).clamp(1024, 256 * 1024);
    let text = if buf.len() > max_bytes {
        let start = buf.len() - max_bytes;
        String::from_utf8_lossy(&buf[start..]).into_owned()
    } else {
        String::from_utf8_lossy(&buf).into_owned()
    };
    Ok(LogTailResponse { cursor: len, text, path: Some(path.to_string_lossy().into_owned()) })
}

/// Parse `http://127.0.0.1:3030/...` into a socket address for readiness checks.
fn dashboard_socket_addr(url: &str) -> Result<std::net::SocketAddr, String> {
    let u = url.trim();
    let rest = u.strip_prefix("http://").or_else(|| u.strip_prefix("https://")).ok_or("URL must start with http:// or https://")?;
    let authority = rest.split(&['/', '?', '#'][..]).next().filter(|s| !s.is_empty()).ok_or("missing host in dashboard URL")?;
    authority.parse().map_err(|e| format!("invalid host:port in URL: {e}"))
}

/// Opens the repository README for setup (browser).
#[tauri::command]
fn open_bridge_documentation() -> Result<(), String> {
    const URL: &str = "https://github.com/kaspanet/rusty-kaspa/blob/master/bridge-tauri/README.md";
    open_os_url(URL)
}

/// Opens the folder containing the desktop executable (e.g. to edit `config.yaml`).
#[tauri::command]
fn reveal_exe_directory() -> Result<(), String> {
    let dir = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = dir.parent().ok_or_else(|| "executable has no parent directory".to_string())?;
    if cfg!(windows) {
        std::process::Command::new("explorer").arg(dir.as_os_str()).spawn().map_err(|e| e.to_string())?;
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(dir).spawn().map_err(|e| e.to_string())?;
    } else {
        std::process::Command::new("xdg-open").arg(dir).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open_os_url(url: &str) -> Result<(), String> {
    if cfg!(windows) {
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(url).spawn().map_err(|e| e.to_string())?;
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn().map_err(|e| e.to_string())?;
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn wait_for_dashboard_http(url: String) -> Result<(), String> {
    let addr = dashboard_socket_addr(&url)?;
    for attempt in 0u32..120 {
        match tokio::net::TcpStream::connect(addr).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                if attempt == 119 {
                    return Err(format!("Dashboard not reachable at {url} after 60s: {e}. Check web_dashboard_port and logs."));
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
    unreachable!("wait loop always returns Ok or Err")
}

fn try_start_from_cli(state: &AppState) {
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        return;
    }
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => e.exit(),
    };
    let url = default_dashboard_iframe_url(&cli);
    match spawn_running_bridge(cli) {
        Ok(running) => {
            if let Ok(mut g) = state.bridge.lock() {
                *g = Some(running);
            }
            eprintln!("rkstratum-bridge-desktop: started bridge (CLI mode). Dashboard: {url}");
        }
        Err(e) => {
            eprintln!("rkstratum-bridge-desktop: failed to start bridge: {e}");
        }
    }
}

fn main() {
    let state = AppState::default();
    try_start_from_cli(&state);

    let menu = Menu::new().add_submenu(Submenu::new(
        "Bridge",
        Menu::new().add_item(CustomMenuItem::new("stop_bridge", "Stop bridge")).add_native_item(tauri::MenuItem::Quit),
    ));

    tauri::Builder::default()
        .menu(menu)
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            is_cli_mode,
            cpu_miner_feature_enabled,
            gui_defaults,
            start_bridge,
            stop_bridge,
            bridge_is_running,
            dashboard_default_url,
            bridge_log_tail,
            wait_for_dashboard_http,
            open_bridge_documentation,
            reveal_exe_directory,
        ])
        .on_menu_event(|event| {
            if event.menu_item_id() == "stop_bridge" {
                let app = event.window().app_handle();
                if let Some(s) = app.try_state::<AppState>() {
                    if let Err(e) = stop_bridge(s) {
                        eprintln!("Stop bridge: {e}");
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                request_bridge_shutdown();
                if let Some(s) = app.try_state::<AppState>() {
                    if let Ok(mut g) = s.bridge.lock() {
                        if let Some(r) = g.take() {
                            let _ = r.join.join();
                        }
                    }
                }
            }
        });
}
