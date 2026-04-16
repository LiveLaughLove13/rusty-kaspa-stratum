## Stratum Bridge Beta

This Stratum Bridge is currently in BETA. Support is available in the Kaspa Discord’s [#mining-and-hardware](https://discord.com/channels/599153230659846165/910178666099646584) channel.

**This documentation** applies to the standalone [**rusty-kaspa-stratum**](https://github.com/LiveLaughLove13/rusty-kaspa-stratum) repository (same `bridge/` layout as [kaspanet/rusty-kaspa `bridge/`](https://github.com/kaspanet/rusty-kaspa/tree/master/bridge)). Repository layout (workspace, AppImage, optional Tauri GUI) is summarized in the repo [`README.md`](../../README.md) and [`docs/PACKAGING.md`](../../docs/PACKAGING.md).

**Issues:** For bugs or features for **this fork**, open an issue on [rusty-kaspa-stratum](https://github.com/LiveLaughLove13/rusty-kaspa-stratum/issues). For **upstream** Kaspa core / full-node topics, use [kaspanet/rusty-kaspa issues](https://github.com/kaspanet/rusty-kaspa/issues) and prefix the title with `[Bridge]` when it concerns the bridge.

The workspace builds the **`stratum-bridge`** binary from the `bridge/` crate (`kaspa-stratum-bridge`).

The bridge can run against:

- **External** node (you run `kaspad` yourself)
- **In-process** node (the bridge starts `kaspad` in the same process)

### Running from a release

If you are running from **GitHub Releases** (without `cargo run`), use the assets published by CI on `v*` tags:

| Asset (examples) | Contents |
| --- | --- |
| `stratum-bridge-linux-amd64.tar.gz` | `stratum-bridge`, `config.yaml`, `README`, `LICENSE` |
| `stratum-bridge-windows-amd64.zip` | `stratum-bridge.exe`, **`rkstratum-bridge-desktop.exe`** (GUI), `config.yaml`, etc. |
| `stratum-bridge-macos-arm64.zip` / `stratum-bridge-macos-amd64.zip` | Same pattern for Apple Silicon vs Intel macOS |
| `stratum-bridge-<version>-x86_64.AppImage.tar.gz` | Linux AppImage (when the release job publishes it) |

Then:

1. Download and extract the archive for your OS.
2. Prepare a config file (for example copy `bridge/config.yaml` from this repository next to the binary, or pass `--config` to your file).
3. Run the bridge binary directly (in-process mode first):

```bash
# Linux/macOS
./stratum-bridge --config bridge/config.yaml --node-mode inprocess -- --utxoindex --rpclisten=127.0.0.1:16110

# Windows (PowerShell)
.\stratum-bridge.exe --config bridge/config.yaml --node-mode inprocess -- --utxoindex --rpclisten=127.0.0.1:16110
```

Then run in external mode:

```bash
./stratum-bridge --config bridge/config.yaml --node-mode external
```

**Linux AppImage (optional):** Releases ship `stratum-bridge-<version>-x86_64.AppImage.tar.gz` only (the AppImage inside
preserves `+x` after `tar -xzf ...` or your archive manager). Extract, then run the `.AppImage`. When launched from a desktop
(no terminal), `AppRun` tries to open a system terminal window so startup logs stay visible; set `RKSTRATUM_NO_AUTO_TERMINAL=1` to
disable that. The AppImage looks for `config.yaml` at `$XDG_CONFIG_HOME/stratum-bridge/config.yaml` (usually
`~/.config/stratum-bridge/config.yaml`) when that file exists; otherwise it uses built-in defaults. Extra CLI arguments are
forwarded to the bridge (an explicit `--config` skips that default). To build the AppImage locally after a musl `stratum-bridge`
release build: `bash bridge/appimage/build.sh <version-label>`.

**Optional desktop GUI:** Windows and macOS release zips may include **`rkstratum-bridge-desktop`** (Tauri shell). Build from source with [`bridge-tauri/README.md`](../../bridge-tauri/README.md).

### CLI Help

For detailed command-line options (from the **repository root** of this workspace):

```bash
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --help
```

This will show all available bridge options and guidance for kaspad arguments.

### Default config / ports

The sample configuration file is:

`bridge/config.yaml`

**Note:** If no config file is found, the bridge uses code defaults:
- Default `kaspad_address`: `localhost:16110` (code default) or `127.0.0.1:16110` (as in `config.yaml`)
- Default `node_mode`: `inprocess` (if `--node-mode` is not specified)
- Default `web_dashboard_port`: empty (dashboard disabled unless configured)

The sample `config.yaml` exposes these Stratum ports:

Each instance also sets a `prom_port`, which is a per-instance Prometheus HTTP endpoint.  
Scrape format: `http://<bridge_host>:<prom_port>/metrics`.

| Port | Purpose |
| --- | --- |
| `:5559` | Stratum listener for very low-difficulty workers (`min_share_diff: 4`), with metrics on Prometheus `:2118`. |
| `:5560` | Stratum listener for low-difficulty workers (`min_share_diff: 512`), with metrics on Prometheus `:2119`. |
| `:5561` | Stratum listener for medium-difficulty workers (`min_share_diff: 1024`), with metrics on Prometheus `:2120`. |
| `:5555` | Stratum listener for higher-difficulty workers (`min_share_diff: 2048`), with metrics on Prometheus `:2114`. |
| `:5556` | Stratum listener for higher-difficulty workers (`min_share_diff: 4096`), with metrics on Prometheus `:2115`. |
| `:5557` | Stratum listener for high-difficulty workers (`min_share_diff: 8192`), with metrics on Prometheus `:2116`. |
| `:5558` | Stratum listener for highest-difficulty workers (`min_share_diff: 16384`), with metrics on Prometheus `:2117`. |

### Run (in-process node, default)

If `--node-mode` is not specified, the bridge defaults to **in-process** mode.

Minimal run (sane defaults, no config file):

```bash
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge
```

Run in-process with explicit config and kaspad args:

```bash
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --config bridge/config.yaml --node-mode inprocess -- --utxoindex --rpclisten=127.0.0.1:16110
```

**Important:** Use `--` separator before kaspad arguments. Arguments starting with hyphens must come after the `--` separator.

**Examples:**
```bash
# ✓ Correct - bridge args first, then --, then kaspad args
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --config config.yaml --node-mode inprocess -- --utxoindex --rpclisten=127.0.0.1:16110

# ✗ Incorrect - will show error message
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --rpclisten=127.0.0.1:16110 --config config.yaml --node-mode inprocess
# Error: tip: to pass '--rpclisten' as a value, use '-- --rpclisten'
```

**Note:** In-process mode uses a separate app directory by default to avoid RocksDB lock conflicts with an existing `kaspad`.

If you want to override it, pass `--appdir` to the bridge (before the `--` separator):

```bash
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --config bridge/config.yaml --node-mode inprocess --appdir "C:\path\to\custom\datadir" -- --utxoindex --rpclisten=127.0.0.1:16110
```

### Run (external node)

Terminal A runs **`kaspad`**. This workspace does **not** include a `kaspad` package binary—build or install `kaspad` from the full [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) tree or use a published `kaspad` release, then start RPC, for example:

```bash
kaspad --utxoindex --rpclisten=127.0.0.1:16110 --rpclisten-borsh=127.0.0.1:17110
```

Terminal B (bridge, **from this repository**):

```bash
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --config bridge/config.yaml --node-mode external
```

### Running two bridges at once (two dashboards)

If you run **two `stratum-bridge` processes** simultaneously (e.g. one in-process and one external),
they **cannot share the same**:
- `web_dashboard_port` (dashboard)
- any Stratum ports
- any per-instance Prometheus ports

Recommended setup:
- **In-process bridge**: run normally with `--config config.yaml` (uses `web_dashboard_port: ":3030"` and the configured instance ports)
- **External bridge**: do **not** reuse the same instance ports; instead, run a single custom Stratum instance on a different port and set a different web dashboard port.

Example (external bridge on `:3031` + Stratum `:16120`):

```bash
cargo run -p kaspa-stratum-bridge --release --features rkstratum_cpu_miner --bin stratum-bridge -- --config config.yaml --web-dashboard-port :3031 --node-mode external --kaspad-address 127.0.0.1:16210 --instance "port=:16120,diff=1" --internal-cpu-miner --internal-cpu-miner-address "kaspatest:address" --internal-cpu-miner-threads 1
```

Open:
- `http://127.0.0.1:3030/` for the in-process bridge
- `http://127.0.0.1:3031/` for the external bridge

### Miner / ASIC connection

- **Pool URL:** `<your_pc_IPv4>:<stratum_port>` (e.g. `192.168.1.10:5555`)
- **Username / wallet:** `kaspa:YOUR_WALLET_ADDRESS.WORKERNAME`

#### Supported Miners

The bridge supports multiple ASIC miner types with automatic detection:

- **IceRiver** (KS2L, KS3M, KS5, etc.): Requires extranonce, single hex string job format
- **Bitmain** (Antminer, GodMiner): No extranonce, array + timestamp job format
- **BzMiner**: Requires extranonce, single hex string job format
- **Goldshell**: Requires extranonce, single hex string job format

The bridge automatically detects miner type and adjusts protocol handling accordingly.

#### Connectivity

To verify connectivity on Windows:

```powershell
netstat -ano | findstr :5555
```

To see detailed miner connection / job logs:

```powershell
$env:RUST_LOG="info,kaspa_stratum_bridge=debug"
```

On Windows, Ctrl+C may show `STATUS_CONTROL_C_EXIT` which is expected.

### Web Dashboard

The bridge includes a built-in web dashboard accessible at the configured `web_dashboard_port`.

**Access:** Open `http://127.0.0.1:3030/` (or your configured port) in a web browser on **the same PC** that runs the bridge.

**Local-first binding:** If you set only a port (`:3030` or `3030`), the dashboard and per-instance **`/metrics`** HTTP servers bind to **`127.0.0.1`**, not the whole network. That keeps the UI and JSON APIs off your LAN/WAN by default—good for a typical home machine. **Stratum** ports (`stratum_port`, `prom_port` when used as port-only) still listen on **all interfaces** (`0.0.0.0`) so miners on your LAN can connect. To open the dashboard from another device, set an explicit address, e.g. `web_dashboard_port: "0.0.0.0:3030"` or your LAN IP.

**Note:** The web dashboard is only started if `web_dashboard_port` is configured (non-empty). The sample `config.yaml` sets this to `:3030` by default. If no config file is used and no `--web-dashboard-port` is specified, the dashboard will not be available.

**Casual users:** Do not set `RKSTRATUM_ALLOW_CONFIG_WRITE=1` unless you understand that it allows changing `config.yaml` via `POST /api/config` with no password (only use on trusted localhost or behind your own protection).

For a structured description of every UI area (header, node panel, trends, blocks, workers, Raw view), see **[UI.md](./UI.md)**.

#### API & metrics (summary)

- **`/metrics`** — Prometheus text format
- **`/api/stats`** — JSON stats (workers, blocks, aggregates)
- **`/api/status`** — Bridge status, nested `node`, optional `host`, flags `host_metrics_enabled` / `geoip_enabled`
- **`/api/host`** — Host snapshot when enabled, or a short JSON message when host metrics are off
- **`/api/config`** — Read/write config when `RKSTRATUM_ALLOW_CONFIG_WRITE=1`

#### Host metrics and optional geo (compile-time + config)

**Default Cargo features** include `rkstratum_geoip`, which pulls in host metrics (`sysinfo`) and the optional geo HTTP client (`ureq`). You do **not** need extra `--features` for a normal `cargo build -p kaspa-stratum-bridge`.

- **Minimal binary** (no host card / no geo client): `cargo build -p kaspa-stratum-bridge --no-default-features`
- **Host only, no geo dependency:** `--no-default-features --features rkstratum_host_metrics`
- **Operator location (manual):** set `RKSTRATUM_LOCATION` to a short string (e.g. city or datacenter); shown in the dashboard **Host** card when host metrics are compiled in.

Approximate **geolocation from the machine’s public IP** is **off** unless you opt in.

- **Config:** set `approximate_geo_lookup: true` in `config.yaml` (top-level, next to other global keys).
- **CLI:** `--approximate-geo-lookup true` or `false` overrides the config file for this run.
- **Web `POST /api/config`:** JSON field `approximate_geo_lookup` updates YAML and the in-process flag (**restart not required** for the running process).
- **URL (optional env):** default is ip-api.com fields-only JSON; set `RKSTRATUM_GEOIP_URL` only if you use your own compatible endpoint. There is **no** `RKSTRATUM_GEOIP` toggle env var—use config, CLI, or the API to enable lookup.
- **Privacy:** enabling geo sends the bridge host’s egress IP to the configured URL; only use on trusted networks or with your own service.

#### Prometheus Metrics

The bridge exposes Prometheus metrics at `/metrics` for integration with monitoring systems:

- Worker share counters and difficulty tracking
- Block mining statistics
- Network hashrate and difficulty
- Worker connection status and uptime
- Internal CPU miner metrics (when feature enabled)

### Variable Difficulty (VarDiff)

The bridge supports automatic difficulty adjustment based on worker performance:

- **Target Shares Per Minute**: Configurable via `shares_per_min` in config
- **Power-of-2 Clamping**: Optional `pow2_clamp` for smoother difficulty transitions
- **Per-Worker Tracking**: Each worker's difficulty is adjusted independently
- **Real-time Display**: Current difficulty shown in web dashboard

VarDiff helps optimize mining efficiency by automatically adjusting difficulty to match each worker's hashrate.

### Internal CPU miner (feature-gated)

The internal CPU miner is a **compile-time feature**.

Build:

```bash
cargo build -p kaspa-stratum-bridge --release --features rkstratum_cpu_miner
```

Run (external node mode + internal CPU miner enabled):

```bash
cargo run -p kaspa-stratum-bridge --release --features rkstratum_cpu_miner --bin stratum-bridge -- --config bridge/config.yaml --node-mode external --internal-cpu-miner --internal-cpu-miner-address kaspa:YOUR_WALLET_ADDRESS --internal-cpu-miner-threads 1
```

### Testing

The package has **two** unit-test targets: the **library** (`src/lib.rs`, e.g. `prom`, hasher) and the **binary** (`src/main.rs`). Omit `--bin` to run both.

Run all bridge tests (including CPU miner tests when feature is enabled):

```bash
cargo test -p kaspa-stratum-bridge --features rkstratum_cpu_miner
```

Run tests without the CPU miner feature:

```bash
cargo test -p kaspa-stratum-bridge
```

Only the binary’s tests (skip library tests such as `prom`):

```bash
cargo test -p kaspa-stratum-bridge --bin stratum-bridge
```

Run a single library test by substring, or use the **full** name with `--exact` (e.g. `prom::tests::test_http_routing_and_config_write`).

```bash
cargo test -p kaspa-stratum-bridge --lib test_http_routing
```

The test suite includes:
- Configuration parsing tests
- JSON-RPC event parsing tests
- Network utilities tests
- Hasher/difficulty calculation tests
- Mining state management tests
- Miner compatibility tests (IceRiver, Bitmain, BzMiner, Goldshell)
- Share validation and PoW checking tests
- VarDiff logic tests
- Wallet address cleaning tests
- CPU miner tests (when `rkstratum_cpu_miner` feature is enabled)

The test suite is comprehensive and educational, with 175+ unit tests designed to help developers understand the codebase.

### Where to change what

For a plain-language guide to what each part of the bridge code does, see [CONTRIBUTOR_MAP.md](CONTRIBUTOR_MAP.md).
