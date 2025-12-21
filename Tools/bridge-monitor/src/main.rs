use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use prometheus_parse::Scrape;
use std::{collections::HashMap, sync::Arc};
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
use tokio::sync::Mutex;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use serde_json::json;

#[cfg(windows)]
mod winproc {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, GetProcessHandleCount, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    pub struct WinExtra {
        pub private_bytes: Option<u64>,
        pub working_set_bytes: Option<u64>,
        pub pagefile_bytes: Option<u64>,
        pub handle_count: Option<u32>,
    }

    pub fn query(pid: u32) -> WinExtra {
        unsafe {
            let desired = PROCESS_QUERY_INFORMATION | PROCESS_VM_READ;
            let handle: HANDLE = OpenProcess(desired, 0, pid);
            if handle == 0 {
                return WinExtra { private_bytes: None, working_set_bytes: None, pagefile_bytes: None, handle_count: None };
            }

            let mut counters: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
            let ok = GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, counters.cb);

            let mut handle_count: u32 = 0;
            let hc_ok = GetProcessHandleCount(handle, &mut handle_count as *mut u32);

            let (private_bytes, working_set_bytes, pagefile_bytes) = if ok == 0 {
                (None, None, None)
            } else {
                (Some(counters.PrivateUsage as u64), Some(counters.WorkingSetSize as u64), Some(counters.PagefileUsage as u64))
            };

            WinExtra {
                private_bytes,
                working_set_bytes,
                pagefile_bytes,
                handle_count: if hc_ok == 0 { None } else { Some(handle_count) },
            }
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "bridge-monitor")]
struct Args {
    #[arg(long)]
    pid: Option<u32>,

    #[arg(long, default_value = "rustbridge")]
    process_name: String,

    #[arg(long, default_value = "2000")]
    interval_ms: u64,

    #[arg(long, default_value = "9001")]
    port: u16,

    #[arg(long)]
    metrics_url: Option<String>,

    #[arg(long, default_value = "http://127.0.0.1:16110")]
    node_url: String,
}

#[derive(serde::Serialize, Debug, Clone)]
struct Metrics {
    ts: String,
    pid: u32,
    cpu_percent: f32,
    rss_bytes: u64,
    virtual_bytes: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    private_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    working_set_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pagefile_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    handle_count: Option<u32>,

    #[serde(flatten)]
    prom_metrics: HashMap<String, f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    network_info: Option<GetInfoResponseMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    network_info_error: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetInfoResponseMessage {
    pub p2p_id: String,
    pub mempool_size: String,
    pub server_version: String,
    pub is_utxo_indexed: bool,
    pub is_synced: bool,
    pub difficulty: f64,
    pub network_name: String,
    pub block_count: String,
    pub header_count: String,
    pub virtual_daa_score: String,
}

struct SharedState {
    pid: Pid,
    monitoring: bool,
}

fn find_pid_by_name(sys: &System, name: &str) -> Option<Pid> {
    let name_lc = name.to_ascii_lowercase();
    sys.processes().iter().find_map(|(pid, p)| {
        let proc_name = p.name().to_string().to_ascii_lowercase();
        if proc_name == name_lc || proc_name == format!("{}.exe", name_lc) {
            Some(*pid)
        } else {
            None
        }
    })
}

async fn scrape_prom_metrics(url: &str) -> HashMap<String, f64> {
    let mut metrics = HashMap::new();
    if let Ok(resp) = reqwest::get(url).await {
        if let Ok(text) = resp.text().await {
            let lines: Vec<_> = text.lines().map(|s| Ok(s.to_string())).collect();
            if let Ok(scrape) = Scrape::parse(lines.into_iter()) {
                for sample in scrape.samples {
                    let value = match sample.value {
                        prometheus_parse::Value::Gauge(g) => g,
                        prometheus_parse::Value::Counter(c) => c,
                        _ => continue, // Skip histograms, summaries, etc.
                    };
                    metrics.insert(sample.metric, value);
                }
            }
        }
    }
    metrics
}

async fn query_kaspa_node(node_url: &str) -> Result<GetInfoResponseMessage> {
    let client = reqwest::Client::new();
    let request = json!({
        "jsonrpc": "2.0",
        "method": "getInfoRequest",
        "params": {},
        "id": 1
    });

    let resp = client.post(node_url).json(&request).send().await?;
    let resp_json: serde_json::Value = resp.json().await?;

    if let Some(err) = resp_json.get("error") {
        return Err(anyhow!("Node RPC error: {}", err));
    }

    let result = resp_json.get("result").ok_or_else(|| anyhow!("Missing 'result' field in node response"))?;
    let info: GetInfoResponseMessage = serde_json::from_value(result.clone())?;
    Ok(info)
}

fn get_metrics(sys: &mut System, pid: Pid, prom_metrics: HashMap<String, f64>, network_info: Option<GetInfoResponseMessage>, network_info_error: Option<String>) -> Option<Metrics> {
    if !sys.refresh_process(pid) {
        return None;
    }
    let proc = sys.process(pid)?;

    let ts = Utc::now().to_rfc3339();
    let cpu_percent = proc.cpu_usage();
    let rss_bytes = proc.memory() * 1024;
    let virtual_bytes = proc.virtual_memory() * 1024;

    #[cfg(windows)]
    let extra = winproc::query(pid.as_u32());

    Some(Metrics {
        ts,
        pid: pid.as_u32(),
        cpu_percent,
        rss_bytes,
        virtual_bytes,
        #[cfg(windows)]
        private_bytes: extra.private_bytes,
        #[cfg(not(windows))]
        private_bytes: None,
        #[cfg(windows)]
        working_set_bytes: extra.working_set_bytes,
        #[cfg(not(windows))]
        working_set_bytes: None,
        #[cfg(windows)]
        pagefile_bytes: extra.pagefile_bytes,
        #[cfg(not(windows))]
        pagefile_bytes: None,
        #[cfg(windows)]
        handle_count: extra.handle_count,
        #[cfg(not(windows))]
        handle_count: None,
        prom_metrics,
        network_info,
        network_info_error,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new().with_cpu().with_memory()),
    );
    sys.refresh_processes();

    let pid = if let Some(pid) = args.pid {
        Pid::from_u32(pid)
    } else {
        find_pid_by_name(&sys, &args.process_name)
            .ok_or_else(|| anyhow!("process '{}' not found; pass --pid", args.process_name))?
    };

    sys.refresh_process(pid);
    tokio::time::sleep(Duration::from_millis(250)).await;

    let sys = Arc::new(Mutex::new(sys));
    let state = Arc::new(Mutex::new(SharedState { pid, monitoring: false }));

    let static_path = warp::path::end().and(warp::fs::file("./Tools/bridge-monitor/monitor-viewer.html"));
    
    let args_for_filter = args.clone();
    let args_filter = warp::any().map(move || args_for_filter.clone());

    let ws_path = warp::path("ws")
        .and(warp::ws())
        .and(with_state(state.clone()))
        .and(with_sys(sys.clone()))
        .and(args_filter)
        .map(|ws: warp::ws::Ws, state, sys, args: Args| {
            ws.on_upgrade(move |socket| handle_client(socket, state, sys, args.interval_ms, args.metrics_url, args.node_url))
        });

    let routes = static_path.or(ws_path);

    println!("Monitor server running at http://127.0.0.1:{}", args.port);
    println!("Monitoring process '{}' (PID: {})", args.process_name, pid);
    if let Some(url) = &args.metrics_url {
        println!("Scraping metrics from: {}", url);
    }

    warp::serve(routes).run(([127, 0, 0, 1], args.port)).await;

    Ok(())
}

async fn handle_client(ws: WebSocket, state: Arc<Mutex<SharedState>>, sys: Arc<Mutex<System>>, interval_ms: u64, metrics_url: Option<String>, node_url: String) {
    let (mut tx, mut rx) = ws.split();

    let state_sender = state.clone();
    let state_receiver = state.clone();

    let sender_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        loop {
            interval.tick().await;
            let (pid, is_monitoring) = {
                let state_lock = state_sender.lock().await;
                (state_lock.pid, state_lock.monitoring)
            };

            if !is_monitoring {
                continue;
            }

            let prom_metrics = if let Some(url) = &metrics_url {
                scrape_prom_metrics(url).await
            } else {
                HashMap::new()
            };

            let (network_info, network_info_error) = match query_kaspa_node(&node_url).await {
                Ok(info) => (Some(info), None),
                Err(err) => (None, Some(err.to_string())),
            };

            let metrics = {
                let mut sys_lock = sys.lock().await;
                get_metrics(&mut sys_lock, pid, prom_metrics, network_info, network_info_error)
            };

            if let Some(metrics) = metrics {
                let msg = serde_json::to_string(&metrics).unwrap();
                if tx.send(Message::text(msg)).await.is_err() {
                    break;
                }
            } else {
                let err_msg = serde_json::json!({ "error": format!("Process with PID {} no longer exists.", pid) }).to_string();
                if tx.send(Message::text(err_msg)).await.is_err() {
                    break;
                }
                break;
            }
        }
    });

    let receiver_task = tokio::spawn(async move {
        while let Some(result) = rx.next().await {
            if let Ok(msg) = result {
                if let Ok(text) = msg.to_str() {
                    let mut state_lock = state_receiver.lock().await;
                    match text {
                        "start" => state_lock.monitoring = true,
                        "stop" => state_lock.monitoring = false,
                        _ => {}
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = sender_task => {},
        _ = receiver_task => {},
    }
}

fn with_state(state: Arc<Mutex<SharedState>>) -> impl Filter<Extract = (Arc<Mutex<SharedState>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn with_sys(sys: Arc<Mutex<System>>) -> impl Filter<Extract = (Arc<Mutex<System>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || sys.clone())
}