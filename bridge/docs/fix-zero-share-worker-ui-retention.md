# Fix: Zero-share workers persisting in terminal and dashboard

Ported from `LiveLaughLove13/rusty-kaspa` branch `fix/zero-share-worker-ui-retention` into this repo (`rusty-kaspa-stratum`).

## Files changed

| Monorepo (`rusty-kaspa`) | This repo (`rusty-kaspa-stratum`) |
|--------------------------|-----------------------------------|
| `bridge/src/share_handler.rs` | `bridge/src/share_handler/lifecycle.rs` |
| `bridge/src/prom.rs` | `bridge/src/prom/metrics.rs` |
| `bridge/src/tests.rs` | `bridge/src/tests.rs` |

## Behavior

- **0-share prune (~180s)** was already present; jobs were undoing it via `get_create_stats` on vardiff paths.
- Job/diffiff paths now use `get_stats_if_exists` only.
- `update_worker_difficulty` no longer calls `ensure_worker_session_metrics` (dashboard activity not reset per job).
- Regression test: `share_handler::lifecycle::retention_tests::set_client_vardiff_does_not_recreate_pruned_stats`.

## Verify

```bash
cargo test -p kaspa-stratum-bridge set_client_vardiff_does_not_recreate
cargo build -p kaspa-stratum-bridge --release --bin stratum-bridge
```
