# Stratum Bridge

This repository contains a standalone Stratum bridge binary:

`stratum-bridge`

The bridge can run against:

- **External** node (you run `kaspad` yourself)
- **In-process** node (the bridge starts `kaspad` in the same process)

The bridge no longer supports spawning `kaspad` as a subprocess.

## Default config / ports

The sample configuration file is:

`config.yaml`

When running from the repository root, pass this full relative path via `--config`.

By default it exposes these Stratum ports:

- `:5555`
- `:5556`
- `:5557`
- `:5558`

## Run (external node)

Terminal A (node):

```bash
cargo run --release --bin kaspad -- --utxoindex --rpclisten=127.0.0.1:16110 --rpclisten-borsh=127.0.0.1:17110
```

Terminal B (bridge):

```bash
cargo run --release --bin stratum-bridge -- --config config.yaml --node-mode external
```

## Run (in-process node)

```bash
cargo run --release --bin stratum-bridge -- --config config.yaml --node-mode inprocess --node-args="--utxoindex --rpclisten=127.0.0.1:16110 --rpclisten-borsh=127.0.0.1:17110"
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

## Desktop UI (optional)

This repository also includes a **Tauri** desktop shell under [`bridge-tauri/`](bridge-tauri/) that embeds the bridge using the `kaspa-stratum-bridge` library from [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) (pinned by git revision in `bridge-tauri/src-tauri/Cargo.toml`). It does not replace the standalone `stratum-bridge` binary above.

See [`bridge-tauri/README.md`](bridge-tauri/README.md) for build commands and dependency notes.

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
