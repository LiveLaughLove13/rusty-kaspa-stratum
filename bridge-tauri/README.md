# RK Stratum Bridge — desktop (Tauri)

Optional GUI that embeds the **same** `kaspa-stratum-bridge` library API as [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) (`bridge/` crate: runner, CLI, embedded dashboard). The standalone `stratum-bridge` binary at this repository root is unchanged and remains the primary way to run the bridge headlessly.

## Prerequisites

- Rust (see `rust-version` in `src-tauri/Cargo.toml`)
- [Tauri 1.x prerequisites](https://v1.tauri.app/v1/guides/getting-started/prerequisites) (platform WebView, etc.)

## Build / check

From the repository root:

```bash
cargo check --manifest-path bridge-tauri/src-tauri/Cargo.toml
```

With the optional internal CPU miner feature (matches `kaspa-stratum-bridge` feature name):

```bash
cargo check --manifest-path bridge-tauri/src-tauri/Cargo.toml --features rkstratum_cpu_miner
```

Release bundle (installer / app — platform-specific):

```bash
cargo tauri build --manifest-path bridge-tauri/src-tauri/Cargo.toml
```

## Kaspa dependency pin

`src-tauri/Cargo.toml` pulls `kaspa-stratum-bridge` and `kaspa-alloc` from **git** at a fixed `rev` on `kaspanet/rusty-kaspa`. Bump that revision when you intentionally upgrade the embedded bridge.

## `serde_nested_with` (crates.io yank)

Upstream Kaspa crates depend on `serde_nested_with`; all releases were **yanked** from crates.io. This crate uses `[patch.crates-io]` in `src-tauri/Cargo.toml` to build the same sources from [murar8/serde_nested_with](https://github.com/murar8/serde_nested_with) (tag `0.2.6`). If upstream Kaspa removes that dependency or republishes the crate, this patch can be dropped.

## Lockfile

`bridge-tauri/src-tauri/Cargo.lock` is generated for this subtree and should be committed so CI and fresh clones resolve dependencies consistently.
