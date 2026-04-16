# Packaging layout (CLI, AppImage, desktop GUI)

This repository mirrors the **kaspanet/rusty-kaspa** bridge layout where it makes sense:

| Path | Role |
| --- | --- |
| `src/` | Standalone `stratum-bridge` binary (CLI). |
| `static/` | Embedded web dashboard assets; `static/assets/kaspa.svg` is the source for the AppImage PNG icon. |
| `appimage/` | Linux **AppImage** scripts — same behavior as `bridge/appimage/` in [rusty-kaspa](https://github.com/kaspanet/rusty-kaspa), but at repo root (`appimage/build.sh` uses `static/assets/kaspa.svg`). |
| `bridge-tauri/` | **RKStratum Bridge** desktop shell (Tauri), the graphical “bridge GUI” that embeds the upstream `kaspa-stratum-bridge` library from rusty-kaspa. |

## Linux AppImage (CLI binary)

Requires a **musl** release build, then:

```bash
cargo build --release --locked --bin stratum-bridge \
  --target x86_64-unknown-linux-musl --features rkstratum_cpu_miner
bash appimage/build.sh "$(git describe --tags --always)"
```

Output: `stratum-bridge-<version>-x86_64.AppImage` at the repository root.

`AppRun` matches upstream: optional terminal relaunch when there is no TTY, default `--config` from `$XDG_CONFIG_HOME/stratum-bridge/config.yaml` when present, `RKSTRATUM_NO_AUTO_TERMINAL=1` to disable the terminal wrapper.

Ship the `.AppImage` inside a `.tar.gz` if you need to preserve the executable bit after download (GitHub’s web UI can strip `+x`).

## Desktop GUI (Tauri)

See [`bridge-tauri/README.md`](../bridge-tauri/README.md). The window title and bundle name are **RKStratum Bridge**, consistent with `bridge-tauri/src-tauri/tauri.conf.json`.
