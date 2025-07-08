# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rezolus is a high-resolution Linux performance telemetry agent written in Rust that provides detailed system performance metrics through efficient, low-overhead instrumentation using eBPF (extended Berkeley Packet Filter).

## Build and Development Commands

```bash
# Format all code (Rust and C)
cargo xtask fmt

# Run linting and checks
cargo check --all-targets
cargo clippy --all-targets --all-features

# Build the project
cargo build --release

# Run tests
cargo test --workspace --tests --bins --locked

# Run the agent (requires sudo for eBPF)
sudo target/release/rezolus config/agent.toml

# Run the exporter
target/release/rezolus exporter config/exporter.toml

# Record metrics to file
target/release/rezolus record --duration 30s http://127.0.0.1:4241 rezolus.parquet

# View recorded metrics
target/release/rezolus view rezolus.parquet
```

## Architecture

### Multi-Mode Operation
- **Agent**: Core telemetry collection with eBPF instrumentation
- **Exporter**: Prometheus-compatible metrics endpoint
- **Recorder**: On-demand metric collection to Parquet files
- **Hindsight**: Rolling metrics buffer for post-incident analysis
- **Viewer**: Web-based dashboard for viewing collected metrics

### Sampler Architecture
- Trait-based design with `Sampler` trait (src/samplers/mod.rs)
- Samplers are registered via `linkme::distributed_slice` for compile-time registration
- Each sampler handles specific metrics domains (CPU, Network, Block I/O, etc.)
- Platform-specific implementations using conditional compilation (`#[cfg(target_os = "linux")]`)

### eBPF Integration
- BPF programs are in `src/bpf/*.bpf.c`
- Built automatically via `build.rs` using `libbpf-cargo`
- Separate programs for different metrics (block_io, cpu, network, syscall, tcp)
- Requires Linux kernel 5.8+ and root privileges

### Key Dependencies
- **Async runtime**: `tokio` for agent HTTP server and concurrent operations
- **Web server**: `axum` for metrics exposition
- **Data formats**: `parquet` and `arrow` for efficient metric storage
- **Metrics**: `metriken` for internal metrics collection
- **eBPF**: `libbpf-rs` and `libbpf-sys` for kernel instrumentation

## Development Notes

### When modifying samplers:
1. Check the `Sampler` trait in src/samplers/mod.rs
2. Look at existing samplers for patterns (e.g., src/samplers/cpu/mod.rs)
3. Register new samplers using the `SAMPLERS` distributed slice
4. Platform-specific code should use conditional compilation

### When working with BPF programs:
1. BPF C code is in src/bpf/*.bpf.c
2. Changes require recompilation (happens automatically via build.rs)
3. Test on a Linux system with kernel 5.8+
4. Must run with sudo/root for eBPF loading

### Configuration files:
- Agent: config/agent.toml
- Exporter: config/exporter.toml  
- Hindsight: config/hindsight.toml

### Package building:
- Debian packages: `debian/package.sh` (runs in Docker)
- RPM packages: `cargo generate-rpm` after release build
- Systemd services are included in packages