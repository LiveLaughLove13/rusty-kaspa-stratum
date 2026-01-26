# Bridge Configuration Options

## RKStratum 2.0 — What’s New (vs CustomIdentifier)

This is a user-facing summary of the main upgrades that landed in the `RKStratum2.0` branch compared to `CustomIdentifier`.

## Web UI & API

- **Built-in web dashboard**
  - Served directly by the bridge (no external UI service required).
  - Includes a main dashboard and a raw view.
- **New HTTP API endpoints for monitoring**
  - `/api/status` for basic node/bridge status.
  - `/api/stats` for summarized worker/block/share stats.
  - `/metrics` for Prometheus scraping.

## CLI & Configuration Quality-of-Life

- **Full CLI support for config values**
  - Most `config.yaml` settings can now be overridden via CLI flags.
- **`--config <path>` support + smarter config discovery**
  - The bridge can locate config relative to the executable in common deployment layouts.
- **More automation-friendly argument parsing**
  - Boolean parsing supports common forms (`true/false`, `1/0`, `yes/no`, `on/off`).
  - Port inputs accept `:PORT` or `HOST:PORT`.

## Multi-Instance Improvements

- **Improved multi-instance configuration & overrides**
  - Adds `--instance` syntax for defining multiple stratum listeners from the CLI.
  - Adds validation for duplicate ports and missing required instance fields.

## Observability & Metrics

- **Expanded Prometheus metrics + better dashboard data**
  - More complete worker/share/block instrumentation and better aggregation.
- **Optional global Web UI bind**
  - A single port can expose aggregated stats across all instances.

## Reliability / Operator Experience

- **Graceful shutdown**
  - Coordinated shutdown with a drain window and forced-exit behavior on repeated interrupts.
- **Logging fixes and improvements**
  - Log files are no longer created empty.
  - Log file naming standardized to `RKStratum_*.log`.
  - Improved console formatting (including better readability of periodic stats output).
- **Health check server**
  - Optional lightweight health endpoint via `health_check_port`.

## Optional / Advanced Features

- **Internal CPU miner (feature-gated)**
  - Optional built-in CPU miner mode controlled via CLI flags when compiled with the `rkstratum_cpu_miner` feature.

## Packaging & Deployment

- **Stratum bridge Docker support**
  - Adds a dedicated `Dockerfile.stratum-bridge`.
- **Release packaging includes the bridge binary**
  - Linux/Windows/macOS release assets build and ship `stratum-bridge` alongside other binaries.

## Quick Start

- **Config file**
  - Default: `config.yaml` (current directory).
  - Repository example: `bridge/config.yaml`.
- **Run (external node)**
  - `stratum-bridge --config bridge/config.yaml --node-mode external`
- **Run (in-process node)**
  - `stratum-bridge --config bridge/config.yaml --node-mode inprocess -- <kaspad args...>`
- **Dashboard (optional)**
  - Set `web_port` (or use `--web-port`) and open: `http://127.0.0.1:<PORT>/`

## Global Settings (shared by all instances)

| Setting | Type | Default | Description |
|---|---|---|---|
| `kaspad_address` | String | `"localhost:16110"` | Kaspa node gRPC address. All instances use the same node. Format: `"HOST:PORT"` or `"grpc://HOST:PORT"`. |
| `block_wait_time` | Integer (milliseconds) | `1000` | How long to wait between checking for new block templates. |
| `print_stats` | Boolean | `true` | Print mining statistics to the console. |
| `log_to_file` | Boolean | `true` | Default log-to-file setting (can be overridden per-instance). |
| `health_check_port` | String | `""` (disabled) | Global health check server port. Leave empty to disable. |
| `web_port` | String | `""` (disabled) | Optional global Web UI / aggregated metrics port. Examples: `":3030"`, `"0.0.0.0:3030"`. |
| `var_diff` | Boolean | `true` | Enable variable difficulty (can be overridden per-instance). |
| `shares_per_min` | Integer | `20` | Target shares per minute for variable difficulty (can be overridden per-instance). |
| `var_diff_stats` | Boolean | `false` | Print variable difficulty statistics (can be overridden per-instance). |
| `extranonce_size` | Integer | `0` | Extranonce size (auto-detected per client; this is for backward compatibility). |
| `pow2_clamp` | Boolean | `false` | Enable power-of-2 difficulty clamping (can be overridden per-instance). |
| `coinbase_tag_suffix` | String | `""` (empty) | Optional suffix appended to the fixed base coinbase tag `"RK-Stratum"`. Results in `"RK-Stratum/<suffix>"`. |

---

## Instance Settings (each instance can have its own)

| Setting | Type | Default | Description |
|---|---|---|---|
| `stratum_port` | String | `":5555"` | **Required.** Stratum port for this instance. Must be unique across instances. Can be `":PORT"` or `"HOST:PORT"`. |
| `min_share_diff` | Integer | `8192` | **Required.** Minimum share difficulty for this instance. |
| `prom_port` | String | `None` (disabled) | Optional Prometheus port for this instance. Can be `":PORT"` or `"HOST:PORT"`. |
| `log_to_file` | Boolean | `None` (inherits global) | Optional per-instance log-to-file setting. If not set, uses the global `log_to_file`. |
| `block_wait_time` | Integer (milliseconds) | `None` (inherits global) | Optional per-instance block template polling interval. |
| `extranonce_size` | Integer | `None` (inherits global) | Optional per-instance extranonce size override. |
| `var_diff` | Boolean | `None` (inherits global) | Optional per-instance variable difficulty override. |
| `shares_per_min` | Integer | `None` (inherits global) | Optional per-instance target shares per minute override. |
| `var_diff_stats` | Boolean | `None` (inherits global) | Optional per-instance variable difficulty statistics override. |
| `pow2_clamp` | Boolean | `None` (inherits global) | Optional per-instance power-of-2 difficulty clamping override. |

---

## Notes and Behavior

- **Multi-instance mode**: If the `instances` array exists in `config.yaml`, the bridge runs in multi-instance mode. Otherwise, it runs in single-instance mode using the global settings.
- **Port formatting**: `stratum_port` and `prom_port` can be specified as `":PORT"` or `"HOST:PORT"`. The bridge will prepend `"0.0.0.0"` if only a port is provided.
- **Coinbase tag**: The base tag is always `"RK-Stratum"`. You can only append a suffix via `coinbase_tag_suffix`. The suffix is sanitized to alphanumeric characters, `.`, `_`, and `-`, and is limited to 64 characters.
- **Variable difficulty**: When enabled, the bridge adjusts the difficulty for miners based on their hashrate to target `shares_per_min`.
- **Logging**: If `log_to_file` is enabled, logs are written under:
  - Windows: `%LOCALAPPDATA%\kaspa-stratum-bridge\logs\RKStratum_<timestamp>.log`
  - Linux/macOS: `~/.kaspa-stratum-bridge/logs/RKStratum_<timestamp>.log`

---

## Configuration Options with CLI Arguments

| Setting | Type | Default | CLI Argument | TODO Status | Description |
|---|---|---|---|---|---|
| `kaspad_address` | String | `"localhost:16110"` | `--kaspad-address <HOST:PORT>` | - | Kaspa node gRPC address. All instances use the same node. |
| `block_wait_time` | Integer (ms) | `1000` | `--block-wait-time <MILLISECONDS>` | - | How long to wait between checking for new block templates. |
| `print_stats` | Boolean | `true` | `--print-stats <true\|false>` | - | Print mining statistics to the console. |
| `log_to_file` | Boolean | `true` | `--log-to-file <true\|false>` | - | Default log-to-file setting (can be overridden per-instance). |
| `health_check_port` | String | `""` (disabled) | `--health-check-port <PORT>` | - | Global health check server port. Leave empty to disable. |
| `web_port` | String | `""` (disabled) | `--web-port <HOST:PORT>` | - | Global Web UI / aggregated metrics server port (optional). |
| `var_diff` | Boolean | `true` | `--var-diff <true\|false>` | - | Enable variable difficulty (can be overridden per-instance). |
| `shares_per_min` | Integer | `20` | `--shares-per-min <COUNT>` | - | Target shares per minute for variable difficulty. |
| `var_diff_stats` | Boolean | `false` | `--var-diff-stats <true\|false>` | - | Print variable difficulty statistics. |
| `extranonce_size` | Integer | `0` | `--extranonce-size <SIZE>` | - | Extranonce size (auto-detected per client; backward compatibility). |
| `pow2_clamp` | Boolean | `false` | `--pow2-clamp <true\|false>` | - | Enable power-of-2 difficulty clamping. |
| `coinbase_tag_suffix` | String | `""` (empty) | `--coinbase-tag-suffix <SUFFIX>` | **COMPLETED** | Optional suffix appended to `"RK-Stratum"` coinbase tag. |
| `stratum_port` | String | `":5555"` | `--stratum-port <HOST:PORT>` | - | **Required.** Stratum port for this instance. Must be unique. |
| `min_share_diff` | Integer | `8192` | `--min-share-diff <DIFFICULTY>` | - | **Required.** Minimum share difficulty for this instance. |
| `prom_port` | String | `None` (disabled) | `--prom-port <HOST:PORT>` | - | Optional Prometheus port for this instance. |
| `instance_log_to_file` | Boolean | `None` (inherits global) | `--instance-log-to-file <true\|false>` | - | Optional per-instance log-to-file setting. |
| `instance_var_diff` | Boolean | `None` (inherits global) | `--instance-var-diff <true\|false>` | - | Optional per-instance variable difficulty override. |
| `instance_shares_per_min` | Integer | `None` (inherits global) | `--instance-shares-per-min <COUNT>` | - | Optional per-instance target shares per minute override. |
| `instance_var_diff_stats` | Boolean | `None` (inherits global) | `--instance-var-diff-stats <true\|false>` | - | Optional per-instance variable difficulty statistics override. |
| `instance_pow2_clamp` | Boolean | `None` (inherits global) | `--instance-pow2-clamp <true\|false>` | - | Optional per-instance power-of-2 difficulty clamping override. |

---

## CLI Notes

- **Booleans**: Accept `true/false`, `1/0`, `yes/no`, `on/off`.
- **Ports**: Accept both `:PORT` and `HOST:PORT` formats.
- **Config override**: CLI flags override values from `config.yaml`.
- **Multi-instance via CLI**:
  - Use one or more `--instance` specs (example: `--instance "port=:5555,diff=8192"`).
  - When using `--instance`, do not use the single-instance flags `--stratum-port/--prom-port/--instance-*`.

---

# Notes

- For the most up-to-date CLI reference, run `stratum-bridge --help`.
