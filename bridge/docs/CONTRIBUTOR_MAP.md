# Bridge contributor map

This document maps the Stratum bridge deliverables:

- **Rust sources** — every `.rs` file under `bridge/src/` (paths in those tables are relative to `bridge/src/`).
- **AppImage packaging** — files under `bridge/appimage/` used to build the Linux AppImage release (paths relative to `bridge/`).
- **Dashboard static assets** — HTML, CSS, JS, and images under `bridge/static/` served or embedded by the HTTP layer (paths relative to `bridge/`).

## Crate root and binary

| File | What this file does |
|------|----------------------|
| `lib.rs` | Declares library modules, documents the crate layout, and re-exports the public API used by the binary and tests. |
| `bridge_error.rs` | `BridgeError`: typed error for the Stratum listener boundary (wraps `SubmitRunError` today); converted to `Box<dyn Error + Send + Sync>` so `EventHandler` stays object-safe. |
| `main.rs` | `stratum-bridge` entrypoint: loads config, CLI overrides, tracing, health checks, optional in-process node and CPU miner, Prometheus/HTTP, and Stratum listener shutdown. |
| `cli.rs` | Command-line argument definitions and applying CLI overrides onto loaded configuration. |
| `app_dirs.rs` | Resolves application data directories (e.g. config and chain data locations) for the running process. |
| `health_check.rs` | Simple HTTP health endpoint logic for orchestrators and load balancers. |
| `inprocess_node.rs` | Starts and supervises an embedded `kaspad` when the bridge runs in in-process node mode. |
| `tracing_setup.rs` | Initializes the tracing subscriber and log filter from environment and defaults. |
| `tests.rs` | Integration and unit tests compiled with the binary test harness (`main.rs`); exercises JSON-RPC, mining helpers, compatibility paths, and related behavior. |

## Config

| File | What this file does |
|------|----------------------|
| `config/app_config.rs` | YAML-backed bridge configuration types, defaults, and deserialization for instances, Stratum ports, node endpoints, and difficulty-related settings. |

## JSON-RPC

| File | What this file does |
|------|----------------------|
| `jsonrpc/jsonrpc_event.rs` | Stratum JSON-RPC request and response types, method enumeration, and parsing/unmarshaling of wire JSON into Rust values. |

## Mining

| File | What this file does |
|------|----------------------|
| `mining/mining_state.rs` | Per-connection mining state: job storage, job id counter, connect time, and stratum difficulty snapshot used by job dispatch and submit handling. |
| `mining/hasher.rs` | Kaspa difficulty and target math, job header serialization, and helpers to build job parameters for different miner families; includes tests for hashing and targets. |
| `mining/pow_diagnostic.rs` | Diagnostic logging and checks to compare headers, nonces, and PoW results when debugging miner or bridge mismatches. |

## Stratum — server and listener

| File | What this file does |
|------|----------------------|
| `stratum/stratum_server.rs` | Wires the Stratum listener to handler maps, APIs, and block-template notifications; main orchestration for accepting miners and serving RPC; maps `mining.submit` failures through `BridgeError` when boxing for the handler map. |
| `stratum/default_client.rs` | Default handler registration and logging glue so a standard deployment connects the listener to the built-in Stratum method implementations. |
| `stratum/stratum_line_codec.rs` | Framing helpers: strip NULs, detect accidental HTTP on the Stratum port, and buffer or split incoming bytes into lines for JSON-RPC. |
| `stratum/stratum_listener/mod.rs` | `StratumListener` type: owns listener config and stats, starts the TCP accept loop, and exposes listen/stop with optional shutdown coordination. |
| `stratum/stratum_listener/types.rs` | Types for the listener: handler map type, connect/disconnect callbacks, per-listener stats, and `StratumListenerConfig`. |
| `stratum/stratum_listener/listen.rs` | Binds the TCP socket, accepts connections, spawns per-client tasks, and runs the disconnect channel loop until shutdown. |
| `stratum/stratum_listener/client_io/mod.rs` | Module root for per-client I/O; re-exports the function that starts each client’s read loop. |
| `stratum/stratum_listener/client_io/read_loop.rs` | Reads from the socket, applies line codec, parses JSON-RPC, dispatches to method handlers, and handles errors and disconnects for one connection. |

## Stratum — context (per connection)

| File | What this file does |
|------|----------------------|
| `stratum/stratum_context/mod.rs` | `StratumContext`: TCP halves, remote address, identity and extranonce locks, mining state handle, disconnect flag, and constructors/accessors. |
| `stratum/stratum_context/types.rs` | `ErrorDisconnected`, `ClientIdentity` (wallet, worker, app strings), and `ContextSummary` for logging. |
| `stratum/stratum_context/outbound.rs` | Implements sending JSON-RPC events and responses (`reply`, stale/bad/low-diff helpers, notifications) over the write half with logging. |

## Stratum — client handler

| File | What this file does |
|------|----------------------|
| `stratum/client_handler/mod.rs` | `ClientHandler`: implements Stratum methods (`mining.subscribe`, `mining.authorize`, `mining.submit`, etc.) and delegates job sends and share handling. |
| `stratum/client_handler/handshake.rs` | Subscribe/authorize flow: extranonce, subscription responses, and related setup before work is sent. |
| `stratum/client_handler/job_dispatch/mod.rs` | Job dispatch module root: shared constants, big-job regex, and re-exports for difficulty notify and job tasks. |
| `stratum/client_handler/job_dispatch/difficulty.rs` | Builds and sends `mining.set_difficulty` to a client (async spawn) and records Prometheus errors on send failure. |
| `stratum/client_handler/job_dispatch/immediate_job.rs` | Fetches a template after subscribe and sends the first `mining.notify` job for a client, including miner-specific job parameter shapes. |
| `stratum/client_handler/job_dispatch/new_block_job.rs` | Runs when the chain head changes: refreshes templates, sends updated `mining.notify`, applies timeouts for missing wallet, and coordinates difficulty updates. |

## Share handler

| File | What this file does |
|------|----------------------|
| `share_handler/mod.rs` | `ShareHandler` struct: per-instance id, overall and per-worker stats map, duplicate-submit guard, and shared accessors used by Stratum and submit code. |
| `share_handler/kaspa_api_trait.rs` | Trait abstracting block template fetch, block submit, and chain queries so share handling stays independent of concrete RPC wiring. |
| `share_handler/work_stats.rs` | `WorkStats` counters (shares, stale, invalid, blocks, var-diff windows, timestamps) shared by stats printing and Prometheus. |
| `share_handler/duplicate_submit.rs` | In-memory guard for duplicate or overlapping submits: outcomes (accepted, stale, low diff, bad) and TTL-based eviction; includes unit tests. |
| `share_handler/vardiff.rs` | Computes the next suggested difficulty from elapsed time and share rate, with pow-of-two clamping options; includes unit tests. |
| `share_handler/lifecycle.rs` | `ShareHandler` behavior over time: create/get stats, periodic hashrate printing, pruning idle workers, var-diff adjustment task, and related long-running logic. |

## Share handler — `mining.submit` pipeline

| File | What this file does |
|------|----------------------|
| `share_handler/submit/mod.rs` | Submit submodule declarations and `ShareHandler::handle_submit` returning `Result<(), SubmitRunError>`. |
| `share_handler/submit/error.rs` | `SubmitError` and `SubmitRunError`: structured errors for parse/validation, Stratum disconnect, and JSON-RPC reply failures; `BlockSubmitRejection` / `classify_block_submit_error_message` for node RPC text. |
| `share_handler/submit/handle.rs` | Orchestrates parse → duplicate check → PoW loop → finish for one `mining.submit`. |
| `share_handler/submit/parse.rs` | Validates `mining.submit` parameters, resolves the job, merges extranonce into the nonce hex string, parses nonce to `u64`, and builds the duplicate key. |
| `share_handler/submit/duplicate.rs` | If the submit key is duplicate or in-flight, sends the appropriate JSON-RPC response and returns early without running PoW. |
| `share_handler/submit/pow_math.rs` | Pure helpers: pool vs network target comparisons, job-ID workaround (`weak_share_job_advance`, `job_id_workaround_exhausted`, `previous_job_id`); unit tests. |
| `share_handler/submit/pow_step.rs` | `evaluate_job_pow`: single-job PoW snapshot from a template header and nonce (`kaspa_pow` + `calculate_target`); unit tests. |
| `share_handler/submit/pow_loop.rs` | PoW validation loop across job ids when needed, pool difficulty checks, weak-share / job-ID workaround; delegates network-block-found path to `block_submit`. |
| `share_handler/submit/block_submit.rs` | When PoW meets network target: logging, block build, `submit_block`, blue-confirm background task, node reject / duplicate-stale / bad-share Stratum replies and stats; unit tests for submit report / classification. |
| `share_handler/submit/finish.rs` | After PoW: records valid or low-diff shares, updates duplicate outcomes, updates Prometheus counters, and sends the final success reply when applicable. |

## Kaspa node integration

| File | What this file does |
|------|----------------------|
| `kaspa/kaspaapi/mod.rs` | Module root: exposes `KaspaApi` and node-status helpers from submodules. |
| `kaspa/kaspaapi/coinbase_tag.rs` | Builds the optional coinbase tag bytes (prefix plus sanitized suffix) passed into block templates. |
| `kaspa/kaspaapi/node_status.rs` | Global `NODE_STATUS` snapshot (sync, peers, tip, difficulty, etc.) and JSON-friendly types for dashboard `/api/status`. |
| `kaspa/kaspaapi/api/mod.rs` | `KaspaApi` struct and impl: gRPC client, notification receiver, connection state, mining-ready checks, and `KaspaApiTrait` implementation surface. |
| `kaspa/kaspaapi/api/block_submit_guard.rs` | Dedupes recent block submits by hash so the bridge does not spam the node with identical submissions. |
| `kaspa/kaspaapi/api/streams.rs` | Subscribes to new-block-template notifications (and related polling), fans out to bridge listeners, and respects shutdown and sync gating. |
| `kaspa/kaspaapi/api/template_submit.rs` | Block template RPC, balance queries, block color checks, and `submit_block` with sync and dedupe guards. |

## Prometheus and HTTP

| File | What this file does |
|------|----------------------|
| `prom/mod.rs` | Prometheus registry setup, metric handles, and recording functions for workers, shares, blocks, and disconnects. |
| `prom/metrics.rs` | Metric registration, worker counter initialization, and bridge-wide gauge/counter definitions used across the crate. |
| `prom/http/mod.rs` | HTTP submodule root: wires static files, stats JSON, config API, ops access, and serve; re-exports server start helpers; contains HTTP routing tests. |
| `prom/http/ops_access.rs` | Optional `/api/config` hardening (env-gated): bearer token, `X-Rkstratum-Csrf`, localhost-only, POST rate limit; see module docs for variable names. |
| `prom/http/serve.rs` | Binds HTTP for metrics and dashboard, routes requests (`/metrics`, `/api/*`, static assets), passes client `SocketAddr` into the handler for ops checks, and applies baseline JSON security headers. |
| `prom/http/static_files.rs` | Serves dashboard static files for the operator UI from embedded assets and/or the on-disk `bridge/static/` tree (see **Web dashboard static assets** below). |
| `prom/http/config_api.rs` | Read/write bridge configuration over HTTP where enabled, and status paths used by the dashboard. |
| `prom/http/stats_json/mod.rs` | Stats JSON submodule: declares types, parse, and aggregate modules and re-exports stats builders. |
| `prom/http/stats_json/types.rs` | Serde structs for `/api/stats` (totals, workers, blocks, optional internal CPU miner fields, uptime). |
| `prom/http/stats_json/parse.rs` | Parses Prometheus exposition text and labels into structures the aggregator can fold. |
| `prom/http/stats_json/aggregate.rs` | Collects current metrics, merges worker and block entries, applies activity filters, and produces the JSON payload for the dashboard. |

## Host and utilities

| File | What this file does |
|------|----------------------|
| `host/host_metrics.rs` | Optional host-level metrics (CPU, memory, disk where supported) for the dashboard or Prometheus. |
| `util/errors.rs` | Short string codes for worker/bridge error classification and Prometheus `record_worker_error` labels. |
| `util/log_colors.rs` | ANSI color helpers for consistent log categories (validation, block, bridge↔ASIC, etc.). |
| `util/net_utils.rs` | Normalizes bind addresses and ports (e.g. turning a bare port into `0.0.0.0:port`) for listeners and HTTP. |

## Optional internal CPU miner

| File | What this file does |
|------|----------------------|
| `cpu_miner/rkstratum_cpu_miner.rs` | In-process CPU miner (feature `rkstratum_cpu_miner`): connects to Stratum like an external miner and exposes metrics for testing or small deployments. |

## AppImage packaging

Linux release bundles are built from a musl `stratum-bridge` binary via `bash bridge/appimage/build.sh` (see [README.md](README.md)). CI uses the same script in `.github/workflows/deploy.yaml`.

| File | What this file does |
|------|----------------------|
| `appimage/build.sh` | Assembles `StratumBridge.AppDir`, copies the `x86_64-unknown-linux-musl/release/stratum-bridge` binary, installs `AppRun` and desktop metadata, renders the icon from `static/assets/kaspa.svg`, downloads `appimagetool` if needed, and produces `stratum-bridge-<version>-x86_64.AppImage` at the repo root. |
| `appimage/AppRun` | AppImage entrypoint: optionally re-launches the app inside a desktop terminal when there is no TTY (unless `RKSTRATUM_NO_AUTO_TERMINAL=1`), ensures `$XDG_CONFIG_HOME/stratum-bridge` exists, and prepends `--config` pointing at `config.yaml` there when the user did not pass `--config`. |
| `appimage/stratum-bridge.desktop` | Freedesktop entry: application name, comment, icon key, category, and `Terminal=true` for menu integration inside the AppDir. |

## Web dashboard static assets

These files are the source for the operator UI. They ship with the repo and are loaded at runtime from disk or embedded into the binary depending on build configuration (`prom/http/static_files.rs`).

| File | What this file does |
|------|----------------------|
| `static/index.html` | Main dashboard page: layout, navigation, and markup for worker stats, node status, and related panels; pulls Tailwind from CDN and links `static/css/site.css` and dashboard scripts. |
| `static/raw.html` | Lightweight “raw” dashboard variant with simpler layout and a link back to the main UI; shares the same CSS and branding assets. |
| `static/css/site.css` | Custom styles, theme tokens, and layout tweaks on top of Tailwind for the RK-Stratum dashboard. |
| `static/js/dashboard.js` | Client-side logic for the main dashboard: fetching `/api/stats`, `/api/status`, rendering tables and charts, and handling UI refresh and interactions. |
| `static/js/raw.js` | Raw view script: fetches `/api/status`, `/api/stats`, and host metrics, renders JSON in a `<pre>`, and provides clipboard copy with toast feedback. |
| `static/assets/kaspa.svg` | Kaspa logo used in the web UI and as the source for the AppImage PNG icon in `appimage/build.sh`. |

---

For dashboard copy and UI behavior, see [UI.md](UI.md). For running the bridge, AppImage notes, and tests, see [README.md](README.md).
