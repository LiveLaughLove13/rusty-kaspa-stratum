# RK Stratum Bridge — desktop (Tauri)

Optional **bridge GUI** (window title and product name: **RKStratum Bridge**, see `src-tauri/tauri.conf.json`) that embeds this repository’s **`bridge/`** crate (`kaspa-stratum-bridge` — same layout as [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) `bridge/`). The standalone `stratum-bridge` **binary** remains the primary way to run the bridge headlessly.

## Prerequisites

- Rust (see `rust-version` in `src-tauri/Cargo.toml`)
- [Tauri 1.x prerequisites](https://v1.tauri.app/v1/guides/getting-started/prerequisites) (platform WebView, etc.)

## Build / check

`kaspa-stratum-bridge` is a **path** dependency to `../../bridge`. `kaspa-alloc` comes from **git** (`kaspanet/rusty-kaspa`, `branch = "master"`) to match the bridge’s allocator.

The repository root **`Cargo.toml`** is a workspace that includes **`bridge`** and **`bridge-tauri/src-tauri`**. Root `[patch.crates-io]` for **serde_nested_with** applies to all members—do not duplicate patches in this crate.

From the **repository root**:

```bash
cargo check -p rkstratum-bridge-desktop
cargo check -p rkstratum-bridge-desktop --features rkstratum_cpu_miner
```

Release bundle (from `bridge-tauri/`, with npm deps installed):

```bash
cd bridge-tauri
npm ci
npx tauri build --features rkstratum_cpu_miner
```

**Linux (x86_64):** Git tag builds include **`rkstratum-bridge-desktop`** in the **`stratum-bridge-linux-amd64`** release archive (next to the musl `stratum-bridge` CLI). That binary is a **glibc** build linked against WebKitGTK (not musl). Run it on a normal desktop distro with WebKit/GTK installed, or build locally after installing [Tauri 1 Linux dependencies](https://v1.tauri.app/v1/guides/getting-started/prerequisites/).

## Lockfile

Reproducible builds use the **workspace root** [`Cargo.lock`](../Cargo.lock) only. Commit lockfile updates at the repo root when dependencies change.
