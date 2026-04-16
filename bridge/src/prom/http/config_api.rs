//! Web dashboard config path, `/api/status` snapshot fields, and `/api/config` read/write.

use crate::app_config::BridgeConfig;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub(crate) struct WebStatusConfig {
    pub kaspad_address: String,
    pub instances: usize,
}

static WEB_STATUS_CONFIG: OnceLock<parking_lot::RwLock<WebStatusConfig>> = OnceLock::new();
static WEB_CONFIG_PATH: OnceLock<PathBuf> = OnceLock::new();
static WEB_CONFIG_WRITE_LOCK: OnceLock<parking_lot::Mutex<()>> = OnceLock::new();

/// Set which config file `/api/config` reads/writes.
/// If not set, it falls back to `config.yaml` in the current working directory.
pub fn set_web_config_path(path: PathBuf) {
    let _ = WEB_CONFIG_PATH.set(path);
}

pub(super) fn get_web_config_path() -> PathBuf {
    WEB_CONFIG_PATH.get().cloned().unwrap_or_else(|| PathBuf::from("config.yaml"))
}

pub(crate) fn config_write_allowed() -> bool {
    matches!(
        std::env::var("RKSTRATUM_ALLOW_CONFIG_WRITE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES") | Ok("on") | Ok("ON")
    )
}

/// Set best-effort status fields used by `/api/status`.
///
/// This avoids re-reading `config.yaml` from within the web/prom servers (which can be wrong when
/// using `--config` / CLI overrides, and also breaks when the working directory differs).
pub fn set_web_status_config(kaspad_address: String, instances: usize) {
    let lock =
        WEB_STATUS_CONFIG.get_or_init(|| parking_lot::RwLock::new(WebStatusConfig { kaspad_address: "-".to_string(), instances: 1 }));
    *lock.write() = WebStatusConfig { kaspad_address, instances: instances.max(1) };
}

pub(crate) fn get_web_status_config() -> WebStatusConfig {
    WEB_STATUS_CONFIG
        .get_or_init(|| parking_lot::RwLock::new(WebStatusConfig { kaspad_address: "-".to_string(), instances: 1 }))
        .read()
        .clone()
}

/// Get current config as JSON
pub(crate) async fn get_config_json() -> String {
    use std::fs;

    let config_path = get_web_config_path();
    if let Ok(content) = fs::read_to_string(&config_path)
        && let Ok(config) = BridgeConfig::from_yaml(&content)
    {
        // Convert BridgeConfig to JSON for web UI
        // For backward compatibility with single-instance mode UI, show first instance fields
        let first_instance = config.instances.first();

        let json_value = serde_json::json!({
            // Global fields
            "kaspad_address": config.global.kaspad_address,
            "block_wait_time": config.global.block_wait_time.as_millis() as u64,
            "print_stats": config.global.print_stats,
            "log_to_file": config.global.log_to_file,
            "health_check_port": config.global.health_check_port,
            "web_dashboard_port": config.global.web_dashboard_port,
            "var_diff": config.global.var_diff,
            "shares_per_min": config.global.shares_per_min,
            "var_diff_stats": config.global.var_diff_stats,
            "extranonce_size": config.global.extranonce_size,
            "pow2_clamp": config.global.pow2_clamp,
            "approximate_geo_lookup": config.global.approximate_geo_lookup,
            "coinbase_tag_suffix": config.global.coinbase_tag_suffix,
            // Instance fields (from first instance for backward compatibility)
            "stratum_port": first_instance.map(|i| &i.stratum_port),
            "min_share_diff": first_instance.map(|i| i.min_share_diff),
            "prom_port": first_instance.and_then(|i| i.prom_port.as_ref()),
        });

        return serde_json::to_string(&json_value).unwrap_or_else(|_| "{}".to_string());
    }
    "{}".to_string()
}

/// Update config from JSON
pub(crate) async fn update_config_from_json(json_body: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::fs;
    use std::time::Duration;

    let updates: serde_json::Value = serde_json::from_str(json_body)?;
    let config_path = get_web_config_path();
    let _guard = WEB_CONFIG_WRITE_LOCK.get_or_init(|| parking_lot::Mutex::new(())).lock();

    // Read existing config
    let content = fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    let mut config = if content.is_empty() {
        BridgeConfig::default()
    } else {
        BridgeConfig::from_yaml(&content).unwrap_or_else(|_| BridgeConfig::default())
    };

    // Update global fields if provided
    if let Some(addr) = updates.get("kaspad_address").and_then(|v| v.as_str()) {
        config.global.kaspad_address = addr.to_string();
    }
    if let Some(bwt) = updates.get("block_wait_time").and_then(|v| v.as_u64()) {
        config.global.block_wait_time = Duration::from_millis(bwt);
    }
    if let Some(stats) = updates.get("print_stats").and_then(|v| v.as_bool()) {
        config.global.print_stats = stats;
    }
    if let Some(log) = updates.get("log_to_file").and_then(|v| v.as_bool()) {
        config.global.log_to_file = log;
    }
    if let Some(port) = updates.get("health_check_port").and_then(|v| v.as_str()) {
        config.global.health_check_port = port.to_string();
    }
    if let Some(port) = updates.get("web_dashboard_port").and_then(|v| v.as_str()) {
        config.global.web_dashboard_port = crate::net_utils::normalize_port(port);
    }
    if let Some(vd) = updates.get("var_diff").and_then(|v| v.as_bool()) {
        config.global.var_diff = vd;
    }
    if let Some(spm) = updates.get("shares_per_min").and_then(|v| v.as_u64()) {
        config.global.shares_per_min = spm as u32;
    }
    if let Some(vds) = updates.get("var_diff_stats").and_then(|v| v.as_bool()) {
        config.global.var_diff_stats = vds;
    }
    if let Some(ens) = updates.get("extranonce_size").and_then(|v| v.as_u64()) {
        config.global.extranonce_size = ens as u8;
    }
    if let Some(clamp) = updates.get("pow2_clamp").and_then(|v| v.as_bool()) {
        config.global.pow2_clamp = clamp;
    }
    if let Some(suffix) = updates.get("coinbase_tag_suffix") {
        if suffix.is_null() {
            config.global.coinbase_tag_suffix = None;
        } else if let Some(s) = suffix.as_str() {
            let trimmed = s.trim();
            config.global.coinbase_tag_suffix = if trimmed.is_empty() { None } else { Some(trimmed.to_string()) };
        }
    }
    if let Some(geo) = updates.get("approximate_geo_lookup").and_then(|v| v.as_bool()) {
        config.global.approximate_geo_lookup = geo;
        crate::host_metrics::set_geoip_enabled_from_config(geo);
    }

    // Update first instance fields if provided (for single-instance mode compatibility)
    if config.instances.is_empty() {
        config.instances.push(Default::default());
    }
    let instance = &mut config.instances[0];

    if let Some(port) = updates.get("stratum_port").and_then(|v| v.as_str()) {
        instance.stratum_port = crate::net_utils::normalize_port(port);
    }
    if let Some(diff) = updates.get("min_share_diff").and_then(|v| v.as_u64()) {
        instance.min_share_diff = diff as u32;
    }
    if let Some(port) = updates.get("prom_port").and_then(|v| v.as_str()) {
        let normalized = crate::net_utils::normalize_port(port);
        if normalized.is_empty() {
            instance.prom_port = None;
        } else {
            instance.prom_port = Some(normalized);
        }
    } else if updates.get("prom_port").map(|v| v.is_null()).unwrap_or(false) {
        instance.prom_port = None;
    }

    // Convert back to YAML with flattened global fields
    let yaml_content = config.to_yaml().map_err(|e| format!("Failed to serialize config to YAML: {}", e))?;

    // Write to file
    fs::write(config_path, yaml_content)?;

    Ok(())
}
