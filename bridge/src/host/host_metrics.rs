//! Optional host (machine) metrics for the dashboard, behind `rkstratum_host_metrics`.
//! Optional coarse geo lookup when built with `rkstratum_geoip`: enable via `approximate_geo_lookup` in config,
//! CLI `--approximate-geo-lookup`, or `POST /api/config` (see `bridge/docs/README.md`). Optional URL: `RKSTRATUM_GEOIP_URL`.

#[cfg(feature = "rkstratum_host_metrics")]
use parking_lot::Mutex;
use serde::Serialize;
#[cfg(feature = "rkstratum_host_metrics")]
use std::path::{Path, PathBuf};
#[cfg(feature = "rkstratum_host_metrics")]
use std::sync::OnceLock;
#[cfg(feature = "rkstratum_host_metrics")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "rkstratum_host_metrics")]
use std::time::{Duration, Instant};

#[cfg(feature = "rkstratum_host_metrics")]
static HOST_SNAPSHOT: OnceLock<Mutex<Option<HostSnapshot>>> = OnceLock::new();

/// When true, kaspad runs inside this process (`--node-mode inprocess`); per-process stats are combined.
#[cfg(feature = "rkstratum_host_metrics")]
static EMBEDDED_KASPAD: AtomicBool = AtomicBool::new(true);

#[cfg(all(feature = "rkstratum_host_metrics", feature = "rkstratum_geoip"))]
static GEOIP_ENABLED_BY_CONFIG: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "rkstratum_host_metrics")]
static NODE_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

#[cfg(feature = "rkstratum_host_metrics")]
#[derive(Clone, Copy)]
struct NetTotalsSample {
    at: Instant,
    rx: u64,
    tx: u64,
}

#[cfg(feature = "rkstratum_host_metrics")]
static NET_TOTALS_PREV: OnceLock<Mutex<Option<NetTotalsSample>>> = OnceLock::new();

#[cfg(feature = "rkstratum_host_metrics")]
#[derive(Clone, Copy)]
struct DiskIoSample {
    at: Instant,
    read: u64,
    write: u64,
}

#[cfg(feature = "rkstratum_host_metrics")]
static BRIDGE_DISK_PREV: OnceLock<Mutex<Option<DiskIoSample>>> = OnceLock::new();

#[cfg(feature = "rkstratum_host_metrics")]
#[derive(Clone, Copy)]
struct VolAvailSample {
    at: Instant,
    avail: u64,
}

#[cfg(feature = "rkstratum_host_metrics")]
static VOL_AVAIL_PREV: OnceLock<Mutex<Option<VolAvailSample>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSnapshot {
    pub hostname: Option<String>,
    pub cpu_brand: String,
    pub cpu_logical_count: usize,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    /// Unix-style load average (may be zero on Windows).
    pub load_one: f64,
    pub load_five: f64,
    pub load_fifteen: f64,
    pub global_cpu_usage_percent: f32,
    /// `true` when the node runs in-process; bridge process metrics include kaspad work (no separate PID).
    pub embedded_kaspad: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_pid: Option<u32>,
    /// Share of total host CPU capacity (0–100): `process.cpu_usage() / logical_cpu_count`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_cpu_usage_percent: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_memory_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_virtual_memory_bytes: Option<u64>,
    /// External mode only: separate `kaspad` / `kaspad.exe` process (best match by name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kaspad_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kaspad_cpu_usage_percent: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kaspad_memory_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kaspad_virtual_memory_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_os_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_os_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_kernel_version: Option<String>,
    pub memory_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_rx_bytes_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_tx_bytes_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_disk_read_bytes_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_disk_write_bytes_per_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_data_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_volume_mount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_volume_fs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_volume_disk_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_volume_total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_volume_available_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_volume_used_percent: Option<f32>,
    /// Rate of change of **free** bytes on the node data volume (negative ≈ space being consumed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_volume_free_bytes_per_sec: Option<f64>,
    /// Operator-provided label (`RKSTRATUM_LOCATION`).
    pub operator_location: Option<String>,
    /// Approximate geo from public IP lookup when `rkstratum_geoip` and config or env enables it.
    pub geo_location: Option<String>,
    pub last_updated_unix_ms: u64,
}

/// `true` when this binary was built with `rkstratum_host_metrics`.
pub const fn host_metrics_compiled() -> bool {
    cfg!(feature = "rkstratum_host_metrics")
}

/// `true` when this binary was built with `rkstratum_geoip` (adds HTTP client for optional lookup).
pub const fn geoip_compiled() -> bool {
    cfg!(feature = "rkstratum_geoip")
}

/// Set from parsed `config.yaml` (`approximate_geo_lookup`). Call once at startup before host metrics run.
pub fn set_geoip_enabled_from_config(enabled: bool) {
    #[cfg(feature = "rkstratum_geoip")]
    {
        GEOIP_ENABLED_BY_CONFIG.store(enabled, Ordering::Relaxed);
    }
    #[cfg(not(feature = "rkstratum_geoip"))]
    {
        let _ = enabled;
    }
}

/// Runtime: geo lookup when compiled in and enabled via config / CLI / `set_geoip_enabled_from_config` (e.g. after `POST /api/config`).
pub fn geoip_runtime_enabled() -> bool {
    #[cfg(feature = "rkstratum_geoip")]
    {
        GEOIP_ENABLED_BY_CONFIG.load(Ordering::Relaxed)
    }
    #[cfg(not(feature = "rkstratum_geoip"))]
    {
        false
    }
}

/// Geo-IP lookups are active (compiled + enabled flag).
pub fn geoip_effective() -> bool {
    geoip_compiled() && geoip_runtime_enabled()
}

/// Call from `main` after CLI/config node mode is known (before host metrics task runs).
/// When `true` (in-process node), kaspad shares this process; the dashboard shows one combined process line.
pub fn set_embedded_kaspad(embedded: bool) {
    #[cfg(feature = "rkstratum_host_metrics")]
    {
        EMBEDDED_KASPAD.store(embedded, Ordering::Relaxed);
    }
    #[cfg(not(feature = "rkstratum_host_metrics"))]
    {
        let _ = embedded;
    }
}

/// Optional Kaspa **datadir** used to pick a volume for storage / I/O context (in-process `--appdir`, or set manually).
pub fn set_node_data_path(path: Option<PathBuf>) {
    #[cfg(feature = "rkstratum_host_metrics")]
    {
        let cell = NODE_DATA_DIR.get_or_init(|| Mutex::new(None));
        *cell.lock() = path.and_then(|p| std::fs::canonicalize(&p).ok().or(Some(p)));
    }
    #[cfg(not(feature = "rkstratum_host_metrics"))]
    {
        let _ = path;
    }
}

#[cfg(feature = "rkstratum_host_metrics")]
fn embedded_kaspad_runtime() -> bool {
    EMBEDDED_KASPAD.load(Ordering::Relaxed)
}

#[cfg(feature = "rkstratum_host_metrics")]
fn snapshot_lock() -> &'static Mutex<Option<HostSnapshot>> {
    HOST_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

/// Latest host snapshot, if the feature is enabled and at least one refresh completed.
pub fn get_host_snapshot() -> Option<HostSnapshot> {
    #[cfg(feature = "rkstratum_host_metrics")]
    {
        snapshot_lock().lock().clone()
    }
    #[cfg(not(feature = "rkstratum_host_metrics"))]
    {
        None
    }
}

/// Background refresh (~20s). No-op when the feature is disabled.
/// Safe to call from every HTTP/metrics listener start: only one task is spawned per process.
pub fn spawn_host_metrics_task() {
    #[cfg(feature = "rkstratum_host_metrics")]
    {
        use std::sync::Once;
        static START_HOST_METRICS: Once = Once::new();
        START_HOST_METRICS.call_once(|| {
            let _ = snapshot_lock();
            tokio::spawn(async {
                loop {
                    let handle = tokio::task::spawn_blocking(collect_host_snapshot_blocking);
                    match handle.await {
                        Ok(Some(snap)) => *snapshot_lock().lock() = Some(snap),
                        Ok(None) => {}
                        Err(e) => tracing::warn!("host metrics task join error: {}", e),
                    }
                    tokio::time::sleep(Duration::from_secs(20)).await;
                }
            });
            tracing::info!(
                "host metrics collection enabled (rkstratum_host_metrics); refresh every 20s"
            );
        });
    }
}

/// Strip Windows `\\?\` / `\\?\UNC\` prefixes so canonical paths match normal mount points (`C:\`, etc.).
#[cfg(all(feature = "rkstratum_host_metrics", windows))]
fn strip_windows_verbatim_for_volume_match(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if lower.starts_with(r"\\?\unc\") {
        // \\?\UNC\server\share → \\server\share
        format!(r"\\{}", &s[r"\\?\UNC\".len()..])
    } else if lower.starts_with(r"\\?\") {
        s[r"\\?\".len()..].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(all(feature = "rkstratum_host_metrics", not(windows)))]
fn strip_windows_verbatim_for_volume_match(s: &str) -> String {
    s.to_string()
}

/// True if `path` lies on this mount (longest matching mount wins in the caller).
#[cfg(feature = "rkstratum_host_metrics")]
fn path_is_under_mount(path_norm: &str, mount_norm: &str) -> bool {
    if mount_norm.is_empty() {
        return false;
    }
    #[cfg(windows)]
    {
        let p = path_norm.trim_end_matches('\\').to_ascii_lowercase();
        let m = mount_norm.trim_end_matches('\\').to_ascii_lowercase();
        if m.is_empty() || !p.starts_with(&m) {
            return false;
        }
        p.len() == m.len() || p.as_bytes().get(m.len()) == Some(&b'\\')
    }
    #[cfg(not(windows))]
    {
        path_norm.starts_with(mount_norm)
    }
}

#[cfg(feature = "rkstratum_host_metrics")]
fn disk_covering_path<'a>(disks: &'a sysinfo::Disks, path: &Path) -> Option<&'a sysinfo::Disk> {
    let path_s = strip_windows_verbatim_for_volume_match(&path.to_string_lossy());
    disks
        .list()
        .iter()
        .filter(|d| {
            let mp = d.mount_point().to_string_lossy();
            let mount_s = strip_windows_verbatim_for_volume_match(&mp);
            !mount_s.is_empty() && path_is_under_mount(&path_s, &mount_s)
        })
        .max_by_key(|d| d.mount_point().as_os_str().len())
}

#[cfg(feature = "rkstratum_host_metrics")]
fn take_network_rates(rx_sum: u64, tx_sum: u64) -> (Option<f64>, Option<f64>) {
    let now = Instant::now();
    let cell = NET_TOTALS_PREV.get_or_init(|| Mutex::new(None));
    let mut g = cell.lock();
    let out = if let Some(p) = g.as_ref() {
        let dt = now.duration_since(p.at).as_secs_f64().max(1e-3);
        let drx = (rx_sum.saturating_sub(p.rx)) as f64 / dt;
        let dtx = (tx_sum.saturating_sub(p.tx)) as f64 / dt;
        (Some(drx), Some(dtx))
    } else {
        (None, None)
    };
    *g = Some(NetTotalsSample {
        at: now,
        rx: rx_sum,
        tx: tx_sum,
    });
    out
}

#[cfg(feature = "rkstratum_host_metrics")]
fn take_bridge_disk_rates(read_tot: u64, write_tot: u64) -> (Option<f64>, Option<f64>) {
    let now = Instant::now();
    let cell = BRIDGE_DISK_PREV.get_or_init(|| Mutex::new(None));
    let mut g = cell.lock();
    let out = if let Some(p) = g.as_ref() {
        let dt = now.duration_since(p.at).as_secs_f64().max(1e-3);
        let dr = (read_tot.saturating_sub(p.read)) as f64 / dt;
        let dw = (write_tot.saturating_sub(p.write)) as f64 / dt;
        (Some(dr), Some(dw))
    } else {
        (None, None)
    };
    *g = Some(DiskIoSample {
        at: now,
        read: read_tot,
        write: write_tot,
    });
    out
}

#[cfg(feature = "rkstratum_host_metrics")]
fn take_volume_free_rate(avail: u64) -> Option<f64> {
    let now = Instant::now();
    let cell = VOL_AVAIL_PREV.get_or_init(|| Mutex::new(None));
    let mut g = cell.lock();
    let out = if let Some(p) = g.as_ref() {
        let dt = now.duration_since(p.at).as_secs_f64().max(1e-3);
        Some((avail as f64 - p.avail as f64) / dt)
    } else {
        None
    };
    *g = Some(VolAvailSample { at: now, avail });
    out
}

#[cfg(feature = "rkstratum_host_metrics")]
fn collect_host_snapshot_blocking() -> Option<HostSnapshot> {
    use std::thread::sleep;
    use std::time::{SystemTime, UNIX_EPOCH};
    use sysinfo::{
        Disks, MemoryRefreshKind, Networks, Pid, ProcessRefreshKind, ProcessesToUpdate,
        RefreshKind, System,
    };

    let mut sys = System::new_with_specifics(RefreshKind::everything());
    let mut disks = Disks::new_with_refreshed_list();
    disks.refresh();
    let mut networks = Networks::new_with_refreshed_list();
    networks.refresh();

    let prk = ProcessRefreshKind::new().with_cpu().with_disk_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, prk);
    sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_memory_specifics(MemoryRefreshKind::everything());
    sys.refresh_processes_specifics(ProcessesToUpdate::All, prk);
    sys.refresh_cpu_usage();
    networks.refresh();
    disks.refresh();

    let mut rx_sum = 0u64;
    let mut tx_sum = 0u64;
    for (_name, iface) in networks.iter() {
        rx_sum = rx_sum.saturating_add(iface.total_received());
        tx_sum = tx_sum.saturating_add(iface.total_transmitted());
    }
    let (network_rx_bytes_per_sec, network_tx_bytes_per_sec) = take_network_rates(rx_sum, tx_sum);

    let hostname = System::host_name();
    let cpus = sys.cpus();
    let cpu_brand = cpus
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let cpu_logical_count = cpus.len().max(1);
    let n_cpus = cpu_logical_count as f32;

    let load = System::load_average();
    let operator_location = std::env::var("RKSTRATUM_LOCATION")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string());

    let geo_location = try_geo_lookup();

    let last_updated_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let total_mem = sys.total_memory();
    let avail_mem = sys.available_memory();
    let memory_used_bytes = total_mem.saturating_sub(avail_mem);
    let swap_total_bytes = sys.total_swap();
    let swap_used_bytes = sys.used_swap();

    let host_os_name = System::name();
    let host_os_version = System::long_os_version();
    let host_kernel_version = System::kernel_version();

    let data_path = NODE_DATA_DIR.get().and_then(|m| m.lock().clone());
    let (
        node_data_dir,
        node_volume_mount,
        node_volume_fs,
        node_volume_disk_kind,
        node_volume_total_bytes,
        node_volume_available_bytes,
        node_volume_used_percent,
        node_volume_free_bytes_per_sec,
    ) = if let Some(ref p) = data_path {
        if let Some(disk) = disk_covering_path(&disks, p) {
            let total = disk.total_space();
            let available = disk.available_space();
            let used_pct = if total > 0 {
                let used = total.saturating_sub(available);
                Some(((used as f64 / total as f64) * 100.0) as f32)
            } else {
                None
            };
            let fs = disk.file_system().to_string_lossy().trim().to_string();
            let fs_opt = if fs.is_empty() { None } else { Some(fs) };
            let mount = disk.mount_point().to_string_lossy().to_string();
            let mount_opt = if mount.is_empty() { None } else { Some(mount) };
            (
                Some(p.to_string_lossy().to_string()),
                mount_opt,
                fs_opt,
                Some(disk.kind().to_string()),
                Some(total),
                Some(available),
                used_pct,
                take_volume_free_rate(available),
            )
        } else {
            (
                Some(p.to_string_lossy().to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }
    } else {
        (None, None, None, None, None, None, None, None)
    };

    let embedded = embedded_kaspad_runtime();
    let self_pid = Pid::from(std::process::id() as usize);

    let (
        bridge_pid,
        bridge_cpu_usage_percent,
        bridge_memory_bytes,
        bridge_virtual_memory_bytes,
        bridge_disk_read_bytes_per_sec,
        bridge_disk_write_bytes_per_sec,
    ) = sys
        .process(self_pid)
        .map_or((None, None, None, None, None, None), |p| {
            let cpu_raw = p.cpu_usage();
            let pct = (cpu_raw / n_cpus).clamp(0.0, 100.0);
            let du = p.disk_usage();
            let (r_bps, w_bps) =
                take_bridge_disk_rates(du.total_read_bytes, du.total_written_bytes);
            (
                Some(self_pid.as_u32()),
                Some((pct * 10.0).round() / 10.0),
                Some(p.memory()),
                Some(p.virtual_memory()),
                r_bps,
                w_bps,
            )
        });

    let (kaspad_pid, kaspad_cpu_usage_percent, kaspad_memory_bytes, kaspad_virtual_memory_bytes) =
        if embedded {
            (None, None, None, None)
        } else {
            pick_kaspad_process(&sys, self_pid).map_or((None, None, None, None), |p| {
                let cpu_raw = p.cpu_usage();
                let pct = (cpu_raw / n_cpus).clamp(0.0, 100.0);
                (
                    Some(p.pid().as_u32()),
                    Some((pct * 10.0).round() / 10.0),
                    Some(p.memory()),
                    Some(p.virtual_memory()),
                )
            })
        };

    Some(HostSnapshot {
        hostname,
        cpu_brand,
        cpu_logical_count,
        memory_total_bytes: total_mem,
        memory_available_bytes: avail_mem,
        load_one: load.one,
        load_five: load.five,
        load_fifteen: load.fifteen,
        global_cpu_usage_percent: sys.global_cpu_usage(),
        embedded_kaspad: embedded,
        bridge_pid,
        bridge_cpu_usage_percent,
        bridge_memory_bytes,
        bridge_virtual_memory_bytes,
        kaspad_pid,
        kaspad_cpu_usage_percent,
        kaspad_memory_bytes,
        kaspad_virtual_memory_bytes,
        host_os_name,
        host_os_version,
        host_kernel_version,
        memory_used_bytes,
        swap_total_bytes,
        swap_used_bytes,
        network_rx_bytes_per_sec,
        network_tx_bytes_per_sec,
        bridge_disk_read_bytes_per_sec,
        bridge_disk_write_bytes_per_sec,
        node_data_dir,
        node_volume_mount,
        node_volume_fs,
        node_volume_disk_kind,
        node_volume_total_bytes,
        node_volume_available_bytes,
        node_volume_used_percent,
        node_volume_free_bytes_per_sec,
        operator_location,
        geo_location,
        last_updated_unix_ms,
    })
}

#[cfg(feature = "rkstratum_host_metrics")]
fn pick_kaspad_process(sys: &sysinfo::System, self_pid: sysinfo::Pid) -> Option<&sysinfo::Process> {
    let mut best: Option<&sysinfo::Process> = None;
    let mut best_cpu = -1.0f32;
    for p in sys.processes().values() {
        if p.pid() == self_pid {
            continue;
        }
        let name = p.name().to_string_lossy().to_lowercase();
        if name == "kaspad" || name == "kaspad.exe" {
            let c = p.cpu_usage();
            if best.is_none() || c > best_cpu {
                best_cpu = c;
                best = Some(p);
            }
        }
    }
    best
}

#[cfg(all(feature = "rkstratum_host_metrics", feature = "rkstratum_geoip"))]
fn try_geo_lookup() -> Option<String> {
    if !geoip_runtime_enabled() {
        return None;
    }
    static GEOIP_WARN_ONCE: std::sync::Once = std::sync::Once::new();
    GEOIP_WARN_ONCE.call_once(|| {
        tracing::warn!(
            "Approximate geo lookup is enabled: the bridge will query a public geo-IP service \
             (override URL with RKSTRATUM_GEOIP_URL). Avoid on networks where outbound HTTP or revealing egress IP is sensitive."
        );
    });

    let url = std::env::var("RKSTRATUM_GEOIP_URL").unwrap_or_else(|_| {
        "http://ip-api.com/json/?fields=status,message,country,regionName,city".to_string()
    });

    #[derive(serde::Deserialize)]
    struct IpApiMini {
        status: String,
        message: Option<String>,
        country: Option<String>,
        #[serde(rename = "regionName")]
        region_name: Option<String>,
        city: Option<String>,
    }

    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(6))
        .call()
        .ok()?;
    let j: IpApiMini = resp.into_json().ok()?;
    if j.status != "success" {
        tracing::debug!("geoip lookup failed: {:?}", j.message);
        return None;
    }
    let parts = [
        j.city.as_deref(),
        j.region_name.as_deref(),
        j.country.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

#[cfg(all(feature = "rkstratum_host_metrics", not(feature = "rkstratum_geoip")))]
fn try_geo_lookup() -> Option<String> {
    None
}
