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
cargo run -- --container 76217c0cf5211e7414ebdf9fc078b7997612158174744ebf42798c484cc58ac8 --interval 2
```

## Configuration

Create a `config.toml` file:

```toml
containers = ["<docker_container_id>"]
interval_seconds = 1
```

## Requirements

- Linux with cgroupv2 enabled
- Docker containers running
- Root access or appropriate permissions to read cgroup files
