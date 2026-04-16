# RK-Stratum web dashboard (UI reference)

This document describes the **implemented** dashboard UI served from `bridge/static/` when `web_dashboard_port` is configured. Static files: `index.html`, `js/dashboard.js`, `css/site.css`. A separate **Raw** JSON view lives at `raw.html`.

Data comes from periodic polling of `/api/status`, `/api/stats`, and (for charts) fields embedded in those responses—not from extra browser → kaspad RPC calls.

---

## Main header (navigation bar)

- **Brand, status pill, local clock** — Connection state and browser-local time.
- **Bridge** — Tiles: Kaspad version, instances, last update, bridge uptime. **Refresh** and **Clear cache** (clears client cache and reloads).
- **Mining & network** — Blocks mined, total shares, active workers (with optional aggregate worker hashrate), network hashrate, internal CPU miner hashrate/blocks (when built with that feature), network difficulty (Prometheus/metrics gauge), DAG height (metrics).  
  **Note:** Network difficulty in the header is the metrics-derived value; the node panel shows RPC DAG difficulty—they can differ slightly (see hint there).
- **Links** — `raw.html` (JSON debug), GitHub.

---

## Left column — Metrics

- **Wallet search** — Filter workers, recent blocks, and related charts by substring match on wallet. Shows a summary card when a filter is active; **Clear** resets.
- **Collapsible body** — Quick links: `/metrics` (Prometheus text), `/api/stats` (JSON), plus a pointer to **Trends → Long range** for scrape YAML and Prometheus setup.

---

## Center column — Kaspad node

- **Sync pill** — Synced / syncing / unknown from node snapshot.
- **RPC & network** — Connection state and network id.
- **Tiles** — DAG blocks, headers, peers, virtual DAA, sink blue score, mempool (count from node `GetInfo`; hover tooltip explains meaning).
- **Header / DAG alignment** — Progress bar and percentage when headers vs DAG differ.
- **Difficulty (RPC)** — Node DAG difficulty; explanatory note when Prometheus header difficulty is also available.
- **Tip hash** — Full selected tip hash (wrapped), **Copy** button.
- **Node snapshot** — “Updated N ago” from snapshot timestamp.

---

## Trends & analytics (full width)

- **Session / Long range** toggle — Session: live Chart.js series from this tab (~2s refresh with main poll; data is session-only). Long range: operator tools for Prometheus, not in-tab history.
- **Main collapse** — Hides the chart/embed stack; header controls stay visible.
- **Bridge host** (sub-panel, collapsible) — Machine running `stratum-bridge`: hostname, CPU, RAM/swap bars, load, bridge/kaspad process stats when available, optional operator location and approximate geo (feature/config gated). Banner text when host metrics are disabled or the first sample is pending.
- **Long-range history** (sub-panel, visible in Long range mode) — **Prometheus path:** open `/metrics`, open local Prometheus (`:9090`), copy scrape YAML / scrape URL, example PromQL. **Optional:** embed Grafana/Prometheus URL in an iframe (stored in browser).
- **Session charts** (sub-panel, visible in Session mode) — CPU, memory, network, disk I/O, peers & mempool, sync & volume; window selector and **Clear series**. Sub-collapse per section is persisted in `localStorage`.

---

## Recent blocks (full width)

- **Day filter**, **Collapse** (whole panel), **Download CSV**.
- **Charts & summary** (sub-collapse) — Count in view, distinct workers, time span; bar chart (blocks over time); doughnut (by instance / worker). Same filters as the table.
- **Block table** (sub-collapse) — Timestamp, instance, bluescore, worker, wallet (copy), nonce, hash (copy). Row details modal where applicable.

---

## Workers (full width)

- **Collapse**, **Download CSV**.
- **Charts & summary** (sub-collapse) — Aggregate tiles; horizontal bar charts (hashrate, difficulty when present, shares/stale/invalid/blocks, session uptime when present). Internal CPU miner appears as a row when enabled and wallet filter is off.
- **Worker table** (sub-collapse) — From **768px** width up, `table-layout: fixed` + percentage columns fit the panel. Below **1280px**, auxiliary columns (stale, invalid, dup, weak, acc Δ, N-blue, DC, err) hide. **Phones (≤767px):** table switches to `auto` layout with column **min-widths** and **horizontal swipe** so headers do not collapse; Jobs, Bal, Last seen, and Session hide—tap a row for the detail modal. “Session” is session uptime when that column is visible.

---

## Cross-cutting behavior

- **Dashboard URL** — With the default `web_dashboard_port` form `:3030`, the server listens on **127.0.0.1** only; use the browser on the same machine, or set `0.0.0.0:3030` (or your LAN IP) in config if you need access from another device. Stratum mining ports are unchanged (still reachable on the LAN).

- **Collapsible sections** — Many panels remember open/closed state in `localStorage` (except Raw, which defaults collapsed where configured).
- **Toasts** — Copy and cache actions surface short feedback.
- **Chart.js** — Resize when expanding collapsed sections that contain canvases.

---

## Raw view (`raw.html`)

Merged JSON-friendly view of status/stats (and host when present) for support and debugging—not a visual duplicate of the main dashboard.

---

## Related code

| Area | Location |
| --- | --- |
| Node snapshot polling | `bridge/src/kaspaapi.rs` (`NODE_STATUS`, `NodeStatusSnapshot`) |
| HTTP API / JSON shape | `bridge/src/prom.rs` (`WebStatusResponse`, `/api/status`, `/api/stats`) |
| Host metrics | `bridge/src/host_metrics.rs` (feature `rkstratum_host_metrics`) |
| Dashboard markup | `bridge/static/index.html` |
| Dashboard logic | `bridge/static/js/dashboard.js` |
