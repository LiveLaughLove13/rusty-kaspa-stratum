# Documentation index

The **canonical bridge guide** (matching the depth and structure of kaspanet `bridge/docs/README.md`, updated for this repository) lives here:

**[../bridge/docs/README.md](../bridge/docs/README.md)** — Stratum Bridge Beta: releases, CLI, config ports, in-process vs external `kaspad`, two-bridge setups, miners, dashboard, VarDiff, CPU miner feature, tests.

Other files in `docs/`:

| File | Purpose |
| --- | --- |
| [CONFIGURATION.md](CONFIGURATION.md) | YAML keys, defaults, CLI overrides, multi-instance notes |
| [PACKAGING.md](PACKAGING.md) | Workspace layout, AppImage, Tauri paths |

The repository **[README.md](../README.md)** covers cloning, CI release artifacts, and quick start. A **container** build lives at the repo root **[`Dockerfile`](../Dockerfile)** (not documented in duplicate elsewhere—see `CONFIGURATION.md` packaging section).

**PR CI vs tags:** Pushes and PRs run `cargo check`, `clippy`, and `cargo test` for **`kaspa-stratum-bridge` only** (see [`.github/workflows/rust.yml`](../.github/workflows/rust.yml)). **`rkstratum-bridge-desktop`** (especially **Linux** WebKit/GTK) is built on **`v*`** tag workflows together with release assets.
