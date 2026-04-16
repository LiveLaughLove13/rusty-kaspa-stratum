# Packaging layout (CLI, AppImage, desktop GUI)

This repository matches **kaspanet/rusty-kaspa** [`bridge/`](https://github.com/kaspanet/rusty-kaspa/tree/master/bridge): the Stratum bridge **crate and sources** live under `bridge/`, not at the repository root.

| Path | Role |
| --- | --- |
| `Cargo.toml` (root) | **Workspace**: `members = ["bridge", "bridge-tauri/src-tauri"]`, shared `[workspace.dependencies]`, `[patch.crates-io]` for `serde_nested_with`. Single **root `Cargo.lock`** for `stratum-bridge` and `rkstratum-bridge-desktop`. |
| `bridge/` | `kaspa-stratum-bridge` package: `src/`, `static/`, `appimage/`, `config.yaml`, `docs/`. |
| `bridge/appimage/` | Linux **AppImage** scripts (same behavior as upstream `bridge/appimage/`). |
| `bridge-tauri/` | **RKStratum Bridge** Tauri shell; depends on `kaspa-stratum-bridge` via `path = "../../bridge"`. |

## Linux AppImage (CLI binary)

Requires a **musl** release build (same **kaspanet `musl-toolchain`** tarball as `rusty-kaspa` / BridgeGUI), then AppImage packaging.

From the repo root (set `GITHUB_WORKSPACE` locally if unset, e.g. `export GITHUB_WORKSPACE="$(pwd)"`):

```bash
source musl-toolchain/build.sh
cd "$GITHUB_WORKSPACE"
export RUSTFLAGS="$RUSTFLAGS -C link-arg=-Wl,--allow-multiple-definition"
cargo build --release --locked -p kaspa-stratum-bridge \
  --target x86_64-unknown-linux-musl --features rkstratum_cpu_miner
bash bridge/appimage/build.sh "$(git describe --tags --always)"
```

Icon source: `bridge/static/assets/kaspa.svg`.

## Desktop GUI (Tauri)

See [`bridge-tauri/README.md`](../bridge-tauri/README.md). The window title and bundle name are **RKStratum Bridge** (`bridge-tauri/src-tauri/tauri.conf.json`).
