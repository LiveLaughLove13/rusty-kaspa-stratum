# Bridge Monitor

A web-based, live monitoring tool for the `rustbridge` (rusty-kaspa-stratum) process and its associated Kaspa node.

This tool runs a local web server to provide a real-time dashboard with detailed performance metrics, including system resource usage, Prometheus metrics from the bridge, and network information from a Kaspa node.

## Features

- **Live Web Dashboard**: A modern, Kaspa-themed dark UI for real-time data visualization.
- **System Metrics**: Live charts for CPU Usage, Memory (RSS, Virtual, Private, Working Set), and Handle Count.
- **Dynamic Prometheus Integration**: Automatically scrapes, detects, and displays all Gauge and Counter metrics from the bridge's `/metrics` endpoint.
- **Interactive Metric Modals**: Click on any metric card to open a modal with a historical graph and the latest value.
- **Kaspa Network Info**: A dedicated "Network" panel shows live data from a Kaspa node, including block count, difficulty, and sync status.
- **Real-time Updates**: All data is streamed efficiently via WebSockets.
- **Browser Controls**: Start and stop monitoring directly from the dashboard.

## Build

From the repository root:

```powershell
cargo build --release --manifest-path Tools/bridge-monitor/Cargo.toml
```

## Run

1.  **Start the Server**

    From the repository root, run the application. By default, it will look for the `rustbridge` process and attempt to connect to local Kaspa node and Prometheus endpoints.

    ```powershell
    # Basic command
    cargo run --release --manifest-path Tools/bridge-monitor/Cargo.toml
    ```

    You can customize the targets using the arguments below.

2.  **Open the Dashboard**

    Open your web browser and navigate to:

    **[http://127.0.0.1:9001](http://127.0.0.1:9001)**

3.  **Control Monitoring**

    Use the **Start** and **Stop** buttons on the web page to control the live data feed.

## Command-Line Arguments

- `--process-name <NAME>`: The name of the process to monitor (default: `rustbridge`).
- `--pid <PID>`: The Process ID to monitor.
- `--interval-ms <MS>`: The refresh interval in milliseconds (default: `2000`).
- `--port <PORT>`: The port for the web server (default: `9001`).
- `--metrics-url <URL>`: The URL of the `rustbridge` Prometheus `/metrics` endpoint.
- `--node-url <URL>`: The URL of the Kaspa node's JSON-RPC endpoint (default: `http://127.0.0.1:16110`).
