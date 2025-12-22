use anyhow::{anyhow, Context, Result};
use futures_util::future::try_join_all;
use once_cell::sync::Lazy;
use rustbridge::log_colors::LogColors;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use yaml_rust::YamlLoader;

static INSTANCE_REGISTRY: Lazy<StdMutex<HashMap<String, usize>>> = Lazy::new(|| StdMutex::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct InstanceConfig {
    pub stratum_port: String,
    pub min_share_diff: u32,
    pub prom_port: Option<String>,
    pub log_to_file: Option<bool>,
    pub var_diff: Option<bool>,
    pub shares_per_min: Option<u32>,
    pub var_diff_stats: Option<bool>,
    pub pow2_clamp: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct GlobalConfig {
    pub kaspad_address: String,
    pub block_wait_time: Duration,
    pub print_stats: bool,
    pub log_to_file: bool,
    pub health_check_port: String,
    pub var_diff: bool,
    pub shares_per_min: u32,
    pub var_diff_stats: bool,
    pub extranonce_size: u8,
    pub pow2_clamp: bool,
}

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub global: GlobalConfig,
    pub instances: Vec<InstanceConfig>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            kaspad_address: "localhost:16110".to_string(),
            block_wait_time: Duration::from_millis(1000),
            print_stats: true,
            log_to_file: true,
            health_check_port: String::new(),
            var_diff: true,
            shares_per_min: 20,
            var_diff_stats: false,
            extranonce_size: 0,
            pow2_clamp: false,
        }
    }
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            stratum_port: ":5555".to_string(),
            min_share_diff: 8192,
            prom_port: None,
            log_to_file: None,
            var_diff: None,
            shares_per_min: None,
            var_diff_stats: None,
            pow2_clamp: None,
        }
    }
}

impl ServiceConfig {
    pub fn from_yaml(content: &str) -> Result<Self> {
        let docs = YamlLoader::load_from_str(content).context("invalid YAML")?;
        let doc = docs.first().ok_or_else(|| anyhow!("empty YAML document"))?;

        let mut global = GlobalConfig::default();

        if let Some(addr) = doc["kaspad_address"].as_str() {
            global.kaspad_address = addr.to_string();
        }
        if let Some(stats) = doc["print_stats"].as_bool() {
            global.print_stats = stats;
        }
        if let Some(log) = doc["log_to_file"].as_bool() {
            global.log_to_file = log;
        }
        if let Some(port) = doc["health_check_port"].as_str() {
            global.health_check_port = port.to_string();
        }
        if let Some(vd) = doc["var_diff"].as_bool() {
            global.var_diff = vd;
        }
        if let Some(spm) = doc["shares_per_min"].as_i64() {
            global.shares_per_min = spm as u32;
        }
        if let Some(vds) = doc["var_diff_stats"].as_bool() {
            global.var_diff_stats = vds;
        }
        if let Some(ens) = doc["extranonce_size"].as_i64() {
            global.extranonce_size = ens as u8;
        }
        if let Some(clamp) = doc["pow2_clamp"].as_bool() {
            global.pow2_clamp = clamp;
        }
        if let Some(bwt) = doc["block_wait_time"].as_i64() {
            global.block_wait_time = Duration::from_millis(bwt as u64);
        } else if let Some(bwt) = doc["block_wait_time"].as_f64() {
            global.block_wait_time = Duration::from_millis(bwt as u64);
        }

        if let Some(instances_yaml) = doc["instances"].as_vec() {
            let mut instances = Vec::new();
            for (idx, instance_yaml) in instances_yaml.iter().enumerate() {
                let mut instance = InstanceConfig::default();

                if let Some(port) = instance_yaml["stratum_port"].as_str() {
                    instance.stratum_port = if port.starts_with(':') {
                        port.to_string()
                    } else {
                        format!(":{}", port)
                    };
                } else {
                    return Err(anyhow!("Instance {} missing required 'stratum_port'", idx));
                }

                if let Some(diff) = instance_yaml["min_share_diff"].as_i64() {
                    instance.min_share_diff = diff as u32;
                } else {
                    return Err(anyhow!("Instance {} missing required 'min_share_diff'", idx));
                }

                if let Some(port) = instance_yaml["prom_port"].as_str() {
                    instance.prom_port = Some(if port.starts_with(':') {
                        port.to_string()
                    } else {
                        format!(":{}", port)
                    });
                }

                if let Some(log) = instance_yaml["log_to_file"].as_bool() {
                    instance.log_to_file = Some(log);
                }

                if let Some(vd) = instance_yaml["var_diff"].as_bool() {
                    instance.var_diff = Some(vd);
                }

                if let Some(spm) = instance_yaml["shares_per_min"].as_i64() {
                    instance.shares_per_min = Some(spm as u32);
                }

                if let Some(vds) = instance_yaml["var_diff_stats"].as_bool() {
                    instance.var_diff_stats = Some(vds);
                }

                if let Some(clamp) = instance_yaml["pow2_clamp"].as_bool() {
                    instance.pow2_clamp = Some(clamp);
                }

                instances.push(instance);
            }

            if instances.is_empty() {
                return Err(anyhow!("instances array cannot be empty"));
            }

            let mut ports = std::collections::HashSet::new();
            for instance in &instances {
                if !ports.insert(&instance.stratum_port) {
                    return Err(anyhow!("Duplicate stratum_port: {}", instance.stratum_port));
                }
            }

            Ok(ServiceConfig { global, instances })
        } else {
            let mut instance = InstanceConfig::default();

            if let Some(port) = doc["stratum_port"].as_str() {
                instance.stratum_port = if port.starts_with(':') {
                    port.to_string()
                } else {
                    format!(":{}", port)
                };
            }

            if let Some(diff) = doc["min_share_diff"].as_i64() {
                instance.min_share_diff = diff as u32;
            }

            if let Some(port) = doc["prom_port"].as_str() {
                instance.prom_port = Some(if port.starts_with(':') {
                    port.to_string()
                } else {
                    format!(":{}", port)
                });
            }

            instance.log_to_file = Some(global.log_to_file);

            Ok(ServiceConfig {
                global,
                instances: vec![instance],
            })
        }
    }
}

pub struct StratumService {
    pub config: ServiceConfig,
}

impl StratumService {
    pub fn new(config: ServiceConfig) -> Self {
        Self { config }
    }

    pub async fn run(self) -> Result<()> {
        LogColors::init();

        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,rustbridge=info"));

        struct CustomFormatter {
            apply_colors: bool,
        }

        impl<S, N> tracing_subscriber::fmt::format::FormatEvent<S, N> for CustomFormatter
        where
            S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
            N: for<'a> tracing_subscriber::fmt::format::FormatFields<'a> + 'static,
        {
            fn format_event(
                &self,
                ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
                mut writer: tracing_subscriber::fmt::format::Writer<'_>,
                event: &tracing::Event<'_>,
            ) -> std::fmt::Result {
                let level = *event.metadata().level();
                write!(writer, "{:5} ", level)?;

                let target = event.metadata().target();
                let formatted_target = if target.starts_with("rustbridge") {
                    format!("rustbridge{}", target.strip_prefix("rustbridge").unwrap_or(target))
                } else {
                    target.to_string()
                };
                write!(writer, "{}: ", formatted_target)?;

                let mut message_buf = String::new();
                {
                    let mut message_writer = tracing_subscriber::fmt::format::Writer::new(&mut message_buf);
                    ctx.format_fields(message_writer.by_ref(), event)?;
                }
                let original_message = message_buf;

                let mut instance_num: Option<usize> = None;
                if let Some(instance_start) = original_message.find("[Instance ") {
                    if let Some(instance_end) = original_message[instance_start..].find("]") {
                        let instance_id_str = &original_message[instance_start..instance_start + instance_end + 1];
                        if let Ok(registry) = INSTANCE_REGISTRY.lock() {
                            if let Some(&num) = registry.get(instance_id_str) {
                                instance_num = Some(num);
                            }
                        }
                    }
                }

                let has_colored_instance = original_message.contains("\x1b[") && original_message.contains("[Instance ");
                if has_colored_instance && self.apply_colors {
                    write!(writer, "{}", original_message)?;
                    writeln!(writer)?;
                    return Ok(());
                }

                let mut cleaned_message = String::new();
                let mut chars = original_message.chars().peekable();
                while let Some(ch) = chars.next() {
                    if ch == '\x1b' {
                        if chars.peek() == Some(&'[') {
                            chars.next();
                            while let Some(&c) = chars.peek() {
                                if c == 'm' {
                                    chars.next();
                                    break;
                                }
                                chars.next();
                            }
                        }
                    } else {
                        cleaned_message.push(ch);
                    }
                }
                let message = cleaned_message;

                if self.apply_colors {
                    if let Some(inst_num) = instance_num {
                        let color_code = rustbridge::log_colors::LogColors::instance_color_code(inst_num);
                        write!(writer, "{}{}\x1b[0m", color_code, &message)?;
                        writeln!(writer)?;
                        return Ok(());
                    }

                    if let Some(instance_start) = message.find("[Instance ") {
                        if let Some(instance_end) = message[instance_start..].find("]") {
                            let instance_str = &message[instance_start + 10..instance_start + instance_end];
                            if let Ok(inst_num) = instance_str.parse::<usize>() {
                                let color_code = rustbridge::log_colors::LogColors::instance_color_code(inst_num);
                                write!(writer, "{}{}\x1b[0m", color_code, &message)?;
                                writeln!(writer)?;
                                return Ok(());
                            }
                        }
                    }

                    if message.contains("[ASIC->BRIDGE]") {
                        write!(writer, "\x1b[96m{}\x1b[0m", &message)?;
                    } else if message.contains("[BRIDGE->ASIC]") {
                        write!(writer, "\x1b[92m{}\x1b[0m", &message)?;
                    } else if message.contains("[VALIDATION]") {
                        write!(writer, "\x1b[93m{}\x1b[0m", &message)?;
                    } else if message.contains("===== BLOCK") || message.contains("[BLOCK]") {
                        write!(writer, "\x1b[95m{}\x1b[0m", &message)?;
                    } else if message.contains("[API]") {
                        write!(writer, "\x1b[94m{}\x1b[0m", &message)?;
                    } else if message.contains("Error") || message.contains("ERROR") {
                        write!(writer, "\x1b[91m{}\x1b[0m", &message)?;
                    } else if message.contains("----------------------------------") {
                        write!(writer, "\x1b[96m{}\x1b[0m", &message)?;
                    } else if message.contains("initializing bridge") {
                        write!(writer, "\x1b[92m{}\x1b[0m", &message)?;
                    } else if message.contains("Starting RustBridge") {
                        write!(writer, "\x1b[92m{}\x1b[0m", &message)?;
                    } else if message.starts_with("\t") && message.contains(":") {
                        if let Some(colon_pos) = message.find(':') {
                            let label_end = message[colon_pos + 1..].chars().take_while(|c| c.is_whitespace()).count();
                            let label_end_pos = colon_pos + 1 + label_end;
                            let label = &message[..label_end_pos];
                            let value = &message[label_end_pos..];
                            write!(writer, "\x1b[94m{}\x1b[0m{}", label, value)?;
                        } else {
                            write!(writer, "{}", &message)?;
                        }
                    } else {
                        write!(writer, "{}", &message)?;
                    }
                } else {
                    write!(writer, "{}", &message)?;
                }

                writeln!(writer)
            }
        }

        let should_log_to_file = self.config.global.log_to_file
            || self.config.instances.first().and_then(|i| i.log_to_file).unwrap_or(false);

        let _file_guard: Option<tracing_appender::non_blocking::WorkerGuard> = if should_log_to_file {
            use std::time::SystemTime;
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let log_filename = format!("rustbridge_{}.log", timestamp);
            let log_path = std::path::Path::new(".").join(&log_filename);

            let file_appender = tracing_appender::rolling::never(".", &log_filename);
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            eprintln!("Logging to file: {}", log_path.display());

            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(LogColors::should_colorize())
                        .event_format(CustomFormatter { apply_colors: LogColors::should_colorize() }),
                )
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .event_format(CustomFormatter { apply_colors: false }),
                )
                .init();

            Some(guard)
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(LogColors::should_colorize())
                        .event_format(CustomFormatter { apply_colors: LogColors::should_colorize() }),
                )
                .init();

            None
        };

        let instance_count = self.config.instances.len();
        tracing::info!("----------------------------------");
        tracing::info!(
            "initializing bridge ({} instance{})",
            instance_count,
            if instance_count > 1 { "s" } else { "" }
        );
        tracing::info!("\tkaspad:          {} (shared)", self.config.global.kaspad_address);
        tracing::info!("\tblock wait:      {:?}", self.config.global.block_wait_time);
        tracing::info!("\tprint stats:     {}", self.config.global.print_stats);
        tracing::info!("\tvar diff:        {}", self.config.global.var_diff);
        tracing::info!("\tshares per min:  {}", self.config.global.shares_per_min);
        tracing::info!("\tvar diff stats:  {}", self.config.global.var_diff_stats);
        tracing::info!("\tpow2 clamp:      {}", self.config.global.pow2_clamp);
        tracing::info!("\textranonce:      auto-detected per client");
        tracing::info!("\thealth check:    {}", self.config.global.health_check_port);

        for (idx, instance) in self.config.instances.iter().enumerate() {
            tracing::info!("\t--- Instance {} ---", idx + 1);
            tracing::info!("\t  stratum:       {}", instance.stratum_port);
            tracing::info!("\t  min diff:      {}", instance.min_share_diff);
            if let Some(ref prom_port) = instance.prom_port {
                tracing::info!("\t  prom:          {}", prom_port);
            }
            if let Some(log_to_file) = instance.log_to_file {
                tracing::info!("\t  log to file:   {}", log_to_file);
            }
        }
        tracing::info!("----------------------------------");

        if !self.config.global.health_check_port.is_empty() {
            let health_port = self.config.global.health_check_port.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                use tokio::net::TcpListener;

                if let Ok(listener) = TcpListener::bind(&health_port).await {
                    tracing::info!("Health check server started on {}", health_port);
                    loop {
                        if let Ok((mut stream, _)) = listener.accept().await {
                            let mut buffer = [0; 1024];
                            if stream.read(&mut buffer).await.is_ok() {
                                let response = "HTTP/1.1 200 OK\r\n\r\n";
                                let _ = stream.write_all(response.as_bytes()).await;
                            }
                        }
                    }
                }
            });
        }

        let kaspa_api = rustbridge::KaspaApi::new(
            self.config.global.kaspad_address.clone(),
            self.config.global.block_wait_time,
        )
        .await
        .map_err(|e| anyhow!("Failed to create Kaspa API client: {}", e))?;

        let mut instance_handles = Vec::new();

        for (idx, instance_config) in self.config.instances.iter().enumerate() {
            let instance_num = idx + 1;
            let instance = instance_config.clone();
            let global = self.config.global.clone();
            let kaspa_api_clone = Arc::clone(&kaspa_api);
            let is_first_instance = idx == 0;

            if let Some(ref prom_port) = instance.prom_port {
                let prom_port = prom_port.clone();
                let instance_num_prom = instance_num;
                tokio::spawn(async move {
                    if let Err(e) = rustbridge::prom::start_prom_server(&prom_port).await {
                        tracing::error!("[Instance {}] Prometheus server error: {}", instance_num_prom, e);
                    }
                });
            }

            let handle = tokio::spawn(async move {
                let instance_id_str = rustbridge::log_colors::LogColors::format_instance_id(instance_num);
                {
                    if let Ok(mut registry) = INSTANCE_REGISTRY.lock() {
                        registry.insert(instance_id_str.clone(), instance_num);
                    }
                }

                let colored_instance_id = rustbridge::log_colors::LogColors::format_instance_id(instance_num);
                tracing::info!("{} Starting on stratum port {}", colored_instance_id, instance.stratum_port);

                let bridge_config = rustbridge::BridgeConfig {
                    instance_id: instance_id_str.clone(),
                    stratum_port: instance.stratum_port.clone(),
                    kaspad_address: global.kaspad_address.clone(),
                    prom_port: String::new(),
                    print_stats: global.print_stats,
                    log_to_file: instance.log_to_file.unwrap_or(global.log_to_file),
                    health_check_port: String::new(),
                    block_wait_time: global.block_wait_time,
                    min_share_diff: instance.min_share_diff,
                    var_diff: instance.var_diff.unwrap_or(global.var_diff),
                    shares_per_min: instance.shares_per_min.unwrap_or(global.shares_per_min),
                    var_diff_stats: instance.var_diff_stats.unwrap_or(global.var_diff_stats),
                    extranonce_size: global.extranonce_size,
                    pow2_clamp: instance.pow2_clamp.unwrap_or(global.pow2_clamp),
                };

                rustbridge::listen_and_serve(
                    bridge_config,
                    Arc::clone(&kaspa_api_clone),
                    if is_first_instance { Some(kaspa_api_clone) } else { None },
                )
                .await
                .map_err(|e| anyhow!("[Instance {}] Bridge server error: {}", instance_num, e))
            });

            instance_handles.push(handle);
        }

        tracing::info!("All {} instance(s) started, waiting for completion...", instance_count);

        let result = try_join_all(instance_handles).await;

        match result {
            Ok(_) => {
                tracing::info!("All instances completed successfully");
                Ok(())
            }
            Err(e) => {
                tracing::error!("One or more instances failed: {:?}", e);
                Err(anyhow!("Instance error: {:?}", e))
            }
        }
    }
}

pub async fn run_from_config_path(config_path: impl AsRef<Path>) -> Result<()> {
    let config_path = config_path.as_ref();

    let config = if config_path.exists() {
        let content = std::fs::read_to_string(config_path)?;
        ServiceConfig::from_yaml(&content)?
    } else {
        ServiceConfig {
            global: GlobalConfig::default(),
            instances: vec![InstanceConfig::default()],
        }
    };

    StratumService::new(config).run().await
}
