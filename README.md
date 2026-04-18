# Stratum Bridge

This repository tracks **kaspanet/rusty-kaspa**’s [`bridge/`](https://github.com/kaspanet/rusty-kaspa/tree/master/bridge) crate layout: Rust sources and `static/` live under **`bridge/`**, built as package `kaspa-stratum-bridge` from the workspace root.

Binary name:

`stratum-bridge`

The bridge can run against:

- **External** node (you run `kaspad` yourself)
- **In-process** node (the bridge starts `kaspad` in the same process)

The bridge no longer supports spawning `kaspad` as a subprocess.

## Default config / ports

The sample configuration file is:

`bridge/config.yaml`

When running from the repository root, pass this path via `--config` (or copy it elsewhere).

With **no** `config.yaml`, the bridge uses a **single** Stratum listener on **`0.0.0.0:5555`** (see `InstanceConfig::default()` in `bridge/src/config/app_config.rs`).

The **checked-in** sample [`bridge/config.yaml`](bridge/config.yaml) is a **multi-instance** layout (seven listeners). Stratum ports there include **`:5559`**, **`:5560`**, **`:5561`**, and **`:5555`–`:5558`** (each with its own `min_share_diff` and optional `prom_port`). Adjust or trim instances to match your deployment.

## Run (external node)

Terminal A runs **`kaspad`**. This repository’s workspace only builds **`stratum-bridge`** (it does not ship the `kaspad` binary). Build or install `kaspad` from the full [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) tree or from that project’s release assets, then start it with RPC flags your bridge will use, for example:

```bash
kaspad --utxoindex --rpclisten=127.0.0.1:16110 --rpclisten-borsh=127.0.0.1:17110
```

Terminal B (bridge, **from this repository**):

```bash
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --config bridge/config.yaml --node-mode external
```

## Run (in-process node)

```bash
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- --config bridge/config.yaml --node-mode inprocess --node-args="--utxoindex --rpclisten=127.0.0.1:16110 --rpclisten-borsh=127.0.0.1:17110"
```

**Note:** If you already have a `kaspad` running, in-process mode may fail with a DB lock error (RocksDB `meta/LOCK`). Either stop the other `kaspad` or run in-process with a separate app directory, e.g. add to `--node-args`:

```text
--appdir=E:\\rusty-kaspa\\tmp-kaspad-inprocess
```

## Miner / ASIC connection

- **Pool URL:** `<your_pc_ip>:5555` (or whichever `stratum_port` you configured)
- **Username / wallet:** `kaspa:YOUR_WALLET_ADDRESS.WORKERNAME`

To verify connectivity on Windows:

```powershell
netstat -ano | findstr :5555
```

To see detailed miner connection / job logs:

```powershell
$env:RUST_LOG="info,kaspa_stratum_bridge=debug"
```

On Windows, Ctrl+C may show `STATUS_CONTROL_C_EXIT` which is expected.

## GitHub Releases (CI)

Tag pushes matching `v*` (for example `v1.3.0`) run [`.github/workflows/rust.yml`](.github/workflows/rust.yml) and attach binaries to the GitHub Release, including:

- **Linux:** `stratum-bridge-linux-amd64` (tar.gz) and, when the AppImage step succeeds, `stratum-bridge-<tag>-x86_64.AppImage` packaged as a `.tar.gz` (preserves the executable bit).
- **Windows:** `stratum-bridge-windows-amd64.zip` (includes `stratum-bridge.exe` and `rkstratum-bridge-desktop.exe` when built with the CPU-miner feature in CI).
- **macOS:** `stratum-bridge-macos-arm64.zip` (Apple Silicon) and `stratum-bridge-macos-amd64.zip` (Intel; `macos-15-intel` runner).

Pushes to **`main`** and pull requests run check/lint/tests only; they do not upload these release assets.

## Linux AppImage (optional)

Headless **CLI** releases can also be packaged as an AppImage (same scripts as [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) under `bridge/appimage/`). After a musl `stratum-bridge` build, run `bash bridge/appimage/build.sh <version-label>` from the repo root. The AppImage uses `$XDG_CONFIG_HOME/stratum-bridge/config.yaml` when present; see [`docs/PACKAGING.md`](docs/PACKAGING.md).

## Desktop UI — RKStratum Bridge (optional)

The **Tauri** desktop shell (“bridge GUI”) lives under [`bridge-tauri/`](bridge-tauri/). It embeds the local **`bridge/`** crate (`kaspa-stratum-bridge`) via a path dependency (`bridge-tauri/src-tauri/Cargo.toml`). It does not replace the standalone `stratum-bridge` CLI binary above.

See [`bridge-tauri/README.md`](bridge-tauri/README.md) and [`docs/PACKAGING.md`](docs/PACKAGING.md) for build commands and layout.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 💝 Acknowledgments

Special thanks to the following individuals and the entire Kaspa Community for their invaluable contributions, support, and efforts that made this Rust-based Stratum bridge possible:

- **@onemorebsmith**
- **@kaspapulse**
- **@aglov413**
- **@kaffinpx**
- **@coderofstuff**
- **@rdugan**
- **@pbfarmer**
- **@dablacksplash**
- **The Kaspa Community**

Your dedication and collaboration have been instrumental in bringing this project to life. Thank you!

## 💰 Donations

Donations are welcomed but not expected. If you find this project useful and would like to support its development:

```
kaspa:qr5wl2hw4vk374vrnk59jnh64tyj8nvsmax3s0gw5ej2yukwlc3gsuxxc2u0y
```
