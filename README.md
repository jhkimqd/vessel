# Vessel - Docker Container Resource Monitor

A Rust-based tool for monitoring Docker container resource usage via cgroupv2, similar to `docker stats`.

## Features

- Monitor CPU usage percentage
- Track memory usage and limits
- Monitor block I/O statistics
- Configurable container monitoring
- Real-time continuous monitoring

## Usage

```bash
# Monitor specific container
cargo run -- --container nginx --interval 2

# Use configuration file
cargo run -- --config config.toml

# Monitor with custom interval
cargo run -- --container redis --interval 5
```

## Configuration

Create a `config.toml` file:

```toml
containers = ["nginx", "redis", "postgres"]
interval_seconds = 1
output_format = "table"
```

## Requirements

- Linux with cgroupv2 enabled
- Docker containers running
- Root access or appropriate permissions to read cgroup files
