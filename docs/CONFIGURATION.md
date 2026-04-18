# Bridge Configuration Options

## Scope

This file is a **configuration and CLI reference** for the bridge in **this repository** (workspace layout aligned with [kaspanet/rusty-kaspa `bridge/`](https://github.com/kaspanet/rusty-kaspa/tree/master/bridge)). Historical note: much of this behavior first landed on the `RKStratum2.0` branch vs older `CustomIdentifier`-era layouts.

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

## Optional / advanced features

- **Internal CPU miner (feature-gated)**
  - Build with `--features rkstratum_cpu_miner`. CLI flags (when enabled) include `--internal-cpu-miner`, `--internal-cpu-miner-address`, `--internal-cpu-miner-threads`, optional throttle and template poll intervals—see `bridge/src/cli.rs` and **`stratum-bridge --help`** on a feature-enabled binary.

## Packaging & Deployment

- **Stratum bridge Docker support**
  - A multi-stage image is defined at the **repository root** [`Dockerfile`](../Dockerfile) (Alpine-based build + non-root runtime with `tini`).
- **Release packaging**
  - Tag builds (see [`.github/workflows/rust.yml`](../.github/workflows/rust.yml)) ship **`stratum-bridge`** for Linux (musl tarball + optional AppImage), Windows, and macOS. **Linux** release archives also include **`rkstratum-bridge-desktop`** (glibc + WebKitGTK); see [`PACKAGING.md`](PACKAGING.md).

## Quick Start

- **Config file**
  - Default: `config.yaml` (current directory).
  - Repository example: `bridge/config.yaml`.
- **Run (external node)**
  - `stratum-bridge --config bridge/config.yaml --node-mode external`
- **Run (in-process node)**
  - `stratum-bridge --config bridge/config.yaml --node-mode inprocess -- <kaspad args...>` (use `--` before `kaspad` flags that start with `-`).
- **Dashboard (optional)**
  - Set `web_dashboard_port` in YAML or `--web-dashboard-port` on the CLI, then open `http://127.0.0.1:<PORT>/` (or your bind address). Port-only forms like `:3030` bind the dashboard to **loopback** by default; see [`../bridge/docs/README.md`](../bridge/docs/README.md).

## Global Settings (shared by all instances)

| Setting | Type | Default | Description |
|---|---|---|---|
| `kaspad_address` | String | `"localhost:16110"` | Kaspa node gRPC address. All instances use the same node. Format: `"HOST:PORT"` or `"grpc://HOST:PORT"`. |
| `block_wait_time` | Integer (milliseconds) | `1000` | How long to wait between checking for new block templates. |
| `print_stats` | Boolean | `true` | Print mining statistics to the console. |
| `log_to_file` | Boolean | `true` | Default log-to-file setting (can be overridden per-instance). |
| `health_check_port` | String | `""` (disabled) | Global health check server port. Leave empty to disable. |
| `web_dashboard_port` | String | `""` (disabled) | Optional global web dashboard + aggregated HTTP surface. Examples: `":3030"`, `"0.0.0.0:3030"`. Empty disables the dashboard server. |
| `approximate_geo_lookup` | Boolean | `false` | When `true` and built with default features (`rkstratum_geoip`), performs optional HTTP geo lookup from egress IP (privacy/network implications—see `bridge/docs/README.md`). |
| `var_diff` | Boolean | `true` | Enable variable difficulty (can be overridden per-instance). |
| `shares_per_min` | Integer | `20` | Target shares per minute for variable difficulty (can be overridden per-instance). |
| `var_diff_stats` | Boolean | `false` | Print variable difficulty statistics (can be overridden per-instance). |
| `extranonce_size` | Integer | `0` | Extranonce size (auto-detected per client; this is for backward compatibility). |
| `pow2_clamp` | Boolean | `false` | Enable power-of-2 difficulty clamping (can be overridden per-instance). |
| `coinbase_tag_suffix` | String | `""` (empty / omitted) | Optional suffix for the coinbase tag. Stored tag bytes are **`RK-Stratum`** plus optional **`/` + sanitized suffix** (alphanumeric, `.`, `_`, `-`; max 64 chars; see `bridge/src/kaspa/kaspaapi/coinbase_tag.rs`). |

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

- **Multi-instance mode**: If the `instances` array exists in `config.yaml`, the bridge runs in multi-instance mode. Otherwise, it runs in single-instance mode using the global settings plus optional top-level `stratum_port` / `min_share_diff` / `prom_port` (see `BridgeConfigRaw` in `bridge/src/config/app_config.rs`).
- **Block templates (multi-instance)**: In one process, **only the first** Stratum instance starts the gRPC **new-block-template notification** listener on the shared `KaspaApi`. Additional instances still get work via **polling** on their `block_wait_time` interval (`bridge/src/stratum/stratum_server.rs`). Tune `block_wait_time` if you need fresher jobs on those listeners.
- **Port formatting**: `stratum_port` and `prom_port` can be specified as `":PORT"` or `"HOST:PORT"`. The bridge will prepend `"0.0.0.0"` if only a port is provided.
- **Coinbase tag**: The base tag is always `"RK-Stratum"`. You can only append a suffix via `coinbase_tag_suffix`. The suffix is sanitized to alphanumeric characters, `.`, `_`, and `-`, and is limited to 64 characters.
- **Variable difficulty**: When enabled, the bridge adjusts the difficulty for miners based on their hashrate to target `shares_per_min`.
- **Logging**: If `log_to_file` is enabled, logs are written under:
  - Windows: `%LOCALAPPDATA%\kaspa-stratum-bridge\logs\RKStratum_<timestamp>.log`
  - Linux/macOS: `~/.kaspa-stratum-bridge/logs/RKStratum_<timestamp>.log`

---

## Configuration options with CLI arguments

| Setting | Type | Default | CLI argument | Description |
|---|---|---|---|---|
| `kaspad_address` | String | `"localhost:16110"` | `--kaspad-address <HOST:PORT>` | Kaspa node gRPC address. All instances share one `KaspaApi` client. |
| `block_wait_time` | Integer (ms) | `1000` | `--block-wait-time <MILLISECONDS>` | Base interval for block-template polling / ticker fallback. |
| `print_stats` | Boolean | `true` | `--print-stats <true\|false>` | Print mining statistics to the console. |
| `log_to_file` | Boolean | `true` | `--log-to-file <true\|false>` | Default log-to-file setting (per-instance can override). |
| `health_check_port` | String | `""` (disabled) | `--health-check-port <PORT>` | Global health check HTTP port; empty disables. |
| `web_dashboard_port` | String | `""` (disabled) | `--web-dashboard-port <HOST:PORT>` | Global web dashboard + JSON APIs + aggregated routing (see `bridge/docs/README.md`). |
| `approximate_geo_lookup` | Boolean | `false` | `--approximate-geo-lookup <true\|false>` | Opt-in geo HTTP lookup (requires default `rkstratum_geoip` build). |
| `var_diff` | Boolean | `true` | `--var-diff <true\|false>` | Variable difficulty (per-instance override in YAML). |
| `shares_per_min` | Integer | `20` | `--shares-per-min <COUNT>` | VarDiff target shares per minute. |
| `var_diff_stats` | Boolean | `false` | `--var-diff-stats <true\|false>` | VarDiff stats logging. |
| `extranonce_size` | Integer | `0` | `--extranonce-size <SIZE>` | Legacy global hint; extranonce is auto-assigned per miner where applicable. |
| `pow2_clamp` | Boolean | `false` | `--pow2-clamp <true\|false>` | Power-of-2 difficulty clamping. |
| `coinbase_tag_suffix` | String | omitted / empty | `--coinbase-tag-suffix <SUFFIX>` | Optional sanitized suffix after `RK-Stratum/` in the coinbase tag. |
| `stratum_port` | String | `":5555"` | `--stratum-port <HOST:PORT>` | Single-instance YAML / CLI: Stratum listen address. |
| `min_share_diff` | Integer | `8192` | `--min-share-diff <DIFFICULTY>` | Single-instance or default diff when using `--instance` without `diff=` in each spec. |
| `prom_port` | String | `None` (disabled) | `--prom-port <HOST:PORT>` | Optional per-instance Prometheus HTTP in single-instance configs. |
| `instance_log_to_file` | Boolean | `None` (inherits global) | `--instance-log-to-file <true\|false>` | Overrides first (only) instance when exactly one instance is configured. |
| `instance_var_diff` | Boolean | `None` (inherits global) | `--instance-var-diff <true\|false>` | Same restriction as above. |
| `instance_shares_per_min` | Integer | `None` (inherits global) | `--instance-shares-per-min <COUNT>` | Same restriction as above. |
| `instance_var_diff_stats` | Boolean | `None` (inherits global) | `--instance-var-diff-stats <true\|false>` | Same restriction as above. |
| `instance_pow2_clamp` | Boolean | `None` (inherits global) | `--instance-pow2-clamp <true\|false>` | Same restriction as above. |

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
