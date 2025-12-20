# RustBridge

A high-performance Rust implementation of the Kaspa Stratum Bridge, providing seamless mining pool connectivity for Kaspa ASIC miners.

## 🚀 Features

- **Multi-ASIC Support** - Connect multiple ASIC miners simultaneously (IceRiver, Bitmain, BzMiner, Goldshell)
- **Automatic Miner Detection** - Automatically detects miner type and configures extranonce settings (no manual configuration required)
- **Block Finding & Submission** - Full support for block discovery and network submission
- **Network Propagation Monitoring** - Tracks block propagation across the Kaspa network
- **Variable Difficulty** - Automatic difficulty adjustment based on miner performance
- **Prometheus Metrics** - Comprehensive metrics for monitoring and analytics
- **Real-time Statistics** - Terminal and API-based statistics display
- **Production Ready** - Successfully finding and submitting blocks to the Kaspa network

## 📋 Requirements

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs/))
- Kaspa node running with gRPC enabled (default port: 16110)
- ASIC miners supporting Kaspa Stratum protocol

## 🛠️ Quick Start

### 1. Clone the Repository

```bash
git clone <repository-url>
cd RustBridge
```

### 2. Configure

Edit `config.yaml` with your Kaspa node address:

```yaml
kaspad_address: "127.0.0.1:16110" # Your Kaspa node gRPC address
stratum_port: ":5555" # Port for miners to connect
prom_port: ":2114" # Prometheus metrics port
```

### 3. Build

```bash
cargo build --release
```

### 4. Run

```bash
./target/release/rustbridge
```

On Windows:

```powershell
.\target\release\rustbridge.exe
```

## 🐳 Running with Docker

You can run the bridge using the pre-built Docker image available on Docker Hub.

1.  **Create a `config.yaml` file:**
    Make sure you have a `config.yaml` file in your current directory. See the example in the repository for a starting point.

2.  **Run the Docker container:**

    ```sh
    docker run -d \
      --name rusty-kaspa-stratum \
      -v "$PWD/config.yaml:/app/config.yaml" \
      --network host \
      --restart unless-stopped \
      kkluster/rusty-kaspa-stratum:latest
    ```

## ⚙️ Configuration

The bridge is configured via `config.yaml`. Key settings:

| Setting           | Description                     | Default           |
| ----------------- | ------------------------------- | ----------------- |
| `kaspad_address`  | Kaspa node gRPC address         | `127.0.0.1:16110` |
| `stratum_port`    | Port for ASIC miners to connect | `:5555`           |
| `prom_port`       | Prometheus metrics port         | `:2114`           |
| `min_share_diff`  | Minimum share difficulty        | `4096`            |
| `var_diff`        | Enable variable difficulty      | `true`            |
| `shares_per_min`  | Target shares per minute        | `20`              |
| `block_wait_time` | Block template wait time (ms)   | `1000`            |
| `print_stats`     | Print statistics to console     | `true`            |
| `log_to_file`     | Enable file logging             | `true`            |

**Note:** `extranonce_size` is now **automatically detected** per client based on miner type. No manual configuration needed!

## 🔌 Supported Miners

### Fully Supported

- **IceRiver** (KS0, KS1, KS2, KS3, KS3L, KS5, etc.)
- **Bitmain** (GodMiner/Antminer)
- **BzMiner**
- **Goldshell**

### Auto-Detection

The bridge automatically detects miner type from the
No manual configuration required!

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Check code
cargo check
```

## 📈 Connection Limits

- **Theoretical Limit**: ~2.1 billion connections (i32::MAX)
- **Practical Limit**: ~65,535 IceRiver/BzMiner/Goldshell miners (extranonce pool)
- **Bitmain Miners**: Unlimited (don't use extranonce)
- **System Limits**: OS file descriptors, memory, network resources

## 🐛 Troubleshooting

### Miners Not Connecting

- Verify Kaspa node is running and accessible
- Check `kaspad_address` in `config.yaml`
- Ensure firewall allows connections on `stratum_port`

### High Share Rejection Rate

- Adjust `min_share_diff` in `config.yaml` (try 2048 or 4096)
- Enable `var_diff` for automatic difficulty adjustment

### Logging

- Set `log_to_file: true` in `config.yaml` for file logs
- Use `RUST_LOG=debug` environment variable for verbose output
- Logs are saved as `rustbridge_<timestamp>.log`

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 💝 Acknowledgments

Special thanks to the following individuals and the entire Kaspa Community for their invaluable contributions, support, and efforts that made this Rust-based Stratum bridge possible:

- **@onemorebsmith**
- **@kaspapulse**
- **@aglov413**
- **@kaffinpx**
- **@coderofstuff**
- **@rdugan**
- **@pbfarmer**
- **@dablacksplash**
- **The Kaspa Community**

Your dedication and collaboration have been instrumental in bringing this project to life. Thank you!

## 💰 Donations

Donations are welcomed but not expected. If you find this project useful and would like to support its development:

```
kaspa:qr5wl2hw4vk374vrnk59jnh64tyj8nvsmax3s0gw5ej2yukwlc3gsuxxc2u0y
```
