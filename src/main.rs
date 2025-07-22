use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

mod config;
mod monitor;
mod cgroup;

use config::Config;
use monitor::ContainerMonitor;

#[derive(Parser)]
#[command(name = "vessel")]
#[command(about = "Monitor Docker container resource usage via cgroupv2")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
    
    /// Container name or ID to monitor
    #[arg(short = 'n', long)]
    container: Option<String>,
    
    /// Monitoring interval in seconds
    #[arg(short, long, default_value = "1")]
    interval: u64,
    
    /// Output JSON file path
    #[arg(short, long, default_value = "vessel_stats.json")]
    output: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let config = if cli.config.exists() {
        Config::from_file(&cli.config)?
    } else {
        Config::default()
    };
    
    let containers = if let Some(container) = cli.container {
        vec![container]
    } else {
        config.containers
    };
    
    if containers.is_empty() {
        eprintln!("No containers specified. Use --container or provide config.toml file.");
        std::process::exit(1);
    }
    
    let mut monitor = ContainerMonitor::new()?;
    let interval = Duration::from_secs(cli.interval);
    
    // Create or truncate the output file
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&cli.output)
        .await?;
    
    // Write opening bracket for JSON array
    file.write_all(b"[\n").await?;
    let mut first_entry = true;
    
    println!("Monitoring containers and writing to {}", cli.output.display());
    
    loop {
        for container in &containers {
            match monitor.get_stats(container).await {
                Ok(stats) => {
                    if !first_entry {
                        file.write_all(b",\n").await?;
                    }
                    
                    let json = stats.to_json()?;
                    file.write_all(format!("  {}", json).as_bytes()).await?;
                    file.flush().await?;
                    
                    first_entry = false;
                    
                    println!("Updated stats for {}", container);
                }
                Err(e) => {
                    eprintln!("Error monitoring {}: {}", container, e);
                }
            }
        }
        
        time::sleep(interval).await;
    }
}
