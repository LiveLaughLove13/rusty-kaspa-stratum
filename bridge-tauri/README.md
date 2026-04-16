# RK Stratum Bridge — desktop (Tauri)

Optional **bridge GUI** (window title and product name: **RKStratum Bridge**, see `src-tauri/tauri.conf.json`) that embeds this repository’s **`bridge/`** crate (`kaspa-stratum-bridge` — same layout as [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) `bridge/`). The standalone `stratum-bridge` **binary** remains the primary way to run the bridge headlessly.

## Prerequisites

- Rust (see `rust-version` in `src-tauri/Cargo.toml`)
- [Tauri 1.x prerequisites](https://v1.tauri.app/v1/guides/getting-started/prerequisites) (platform WebView, etc.)

## Build / check

`kaspa-stratum-bridge` is a **path** dependency to `../../bridge`. `kaspa-alloc` comes from **git** (`kaspanet/rusty-kaspa`, `branch = "master"`) to match the bridge’s allocator.

The workspace root `Cargo.toml` defines `[patch.crates-io]` for **serde_nested_with** (yanked on crates.io). Building only the Tauri crate may require the same patch in this manifest or building from a workspace that includes the patch.

From the repository root:

```bash
cargo check --manifest-path bridge-tauri/src-tauri/Cargo.toml
cargo check --manifest-path bridge-tauri/src-tauri/Cargo.toml --features rkstratum_cpu_miner
```

Release bundle:

```bash
cargo tauri build --manifest-path bridge-tauri/src-tauri/Cargo.toml
```

## Lockfile

Commit `bridge-tauri/src-tauri/Cargo.lock` when it changes so CI and fresh clones stay reproducible.
