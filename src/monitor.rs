use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::cgroup::CgroupManager;

#[derive(Debug, Clone, Serialize)]
pub struct ContainerStats {
    pub id: String,
    pub name: String,
    pub cpu_percentage: f64,
    pub cpu_usage_usec: u64,
    pub system_usage_usec: u64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percentage: f64,
    pub network_rx: u64,
    pub network_tx: u64,
    pub block_read: u64,
    pub block_write: u64,
    pub timestamp: DateTime<Utc>,
}

impl ContainerStats {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
    
    // pub fn format_output(&self) -> String {
    //     let mem_usage_mb = self.memory_usage as f64 / 1024.0 / 1024.0;
    //     let mem_limit_mb = self.memory_limit as f64 / 1024.0 / 1024.0;
    //     let net_rx_mb = self.network_rx as f64 / 1024.0 / 1024.0;
    //     let net_tx_mb = self.network_tx as f64 / 1024.0 / 1024.0;
    //     let block_read_mb = self.block_read as f64 / 1024.0 / 1024.0;
    //     let block_write_mb = self.block_write as f64 / 1024.0 / 1024.0;
        
    //     format!(
    //         "{:<15}\t{:>6.2}%\t\t{:>6.1}MB / {:.1}MB\t{:>5.2}%\t\t{:.1}MB / {:.1}MB\t{:.1}MB / {:.1}MB",
    //         self.name,
    //         self.cpu_percentage,
    //         mem_usage_mb,
    //         mem_limit_mb,
    //         self.memory_percentage,
    //         net_rx_mb,
    //         net_tx_mb,
    //         block_read_mb,
    //         block_write_mb
    //     )
    // }
}

pub struct ContainerMonitor {
    cgroup_manager: CgroupManager,
    previous_stats: HashMap<String, (u64, u64)>, // (cpu_usage, timestamp_ns)
}

impl ContainerMonitor {
    pub fn new() -> Result<Self> {
        let cgroup_manager = CgroupManager::new()?;
        
        Ok(Self {
            cgroup_manager,
            previous_stats: HashMap::new(),
        })
    }
    
    pub async fn get_stats(&mut self, container_name: &str) -> Result<ContainerStats> {
        let cgroup_path = self.cgroup_manager.find_container_cgroup(container_name)?;
        let container_id = self.get_container_id(container_name)?;
        
        let (cpu_usage_percent, cpu_usage_usec, system_usage_usec) = self.get_cpu_usage(&cgroup_path, container_name)?;
        let (memory_usage, memory_limit, memory_percent) = self.get_memory_usage(&cgroup_path)?;
        let (net_rx, net_tx) = self.get_network_usage()?;
        let (block_read, block_write) = self.get_block_io_usage(&cgroup_path)?;
        
        Ok(ContainerStats {
            id: container_id,
            name: container_name.to_string(),
            cpu_percentage: cpu_usage_percent,
            cpu_usage_usec,
            system_usage_usec,
            memory_usage,
            memory_limit,
            memory_percentage: memory_percent,
            network_rx: net_rx,
            network_tx: net_tx,
            block_read,
            block_write,
            timestamp: Utc::now(),
        })
    }
    
    fn get_container_id(&self, container_name: &str) -> Result<String> {
        // Try using docker inspect to get full container ID
        let output = std::process::Command::new("docker")
            .args(["inspect", "--format", "{{.Id}}", container_name])
            .output()
            .context("Failed to run docker inspect")?;
            
        if output.status.success() {
            let id = String::from_utf8(output.stdout)?
                .trim()
                .to_string();
            return Ok(id);
        }
        
        // If docker command fails, assume it's already a container ID
        Ok(container_name.to_string())
    }
    
    fn get_cpu_usage(&mut self, cgroup_path: &Path, container_name: &str) -> Result<(f64, u64, u64)> {
        let cpu_stat_path = cgroup_path.join("cpu.stat");
        let content = fs::read_to_string(&cpu_stat_path)
            .context(format!("Failed to read {:?}", cpu_stat_path))?;
        
        let mut usage_usec = 0u64;
        let mut system_usec = 0u64;
        
        for line in content.lines() {
            if line.starts_with("usage_usec ") {
                usage_usec = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("system_usec ") {
                system_usec = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        let cpu_percent = if let Some((prev_usage, prev_time)) = self.previous_stats.get(container_name) {
            let usage_diff = usage_usec.saturating_sub(*prev_usage);
            let time_diff_usec = (current_time - prev_time) / 1000; // Convert ns to us
            
            if time_diff_usec > 0 {
                (usage_diff as f64 / time_diff_usec as f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        self.previous_stats.insert(container_name.to_string(), (usage_usec, current_time));
        Ok((cpu_percent, usage_usec, system_usec))
    }
    
    fn get_memory_usage(&self, cgroup_path: &Path) -> Result<(u64, u64, f64)> {
        let memory_current_path = cgroup_path.join("memory.current");
        let memory_max_path = cgroup_path.join("memory.max");
        
        let current = fs::read_to_string(&memory_current_path)
            .context(format!("Failed to read {:?}", memory_current_path))?
            .trim()
            .parse::<u64>()?;
        
        let max_content = fs::read_to_string(&memory_max_path)
            .context(format!("Failed to read {:?}", memory_max_path))?;
        
        let max = if max_content.trim() == "max" {
            // Get system memory as fallback
            self.get_system_memory().unwrap_or(0)
        } else {
            max_content.trim().parse::<u64>()?
        };
        
        let percentage = if max > 0 {
            (current as f64 / max as f64) * 100.0
        } else {
            0.0
        };
        
        Ok((current, max, percentage))
    }
    
    fn get_network_usage(&self) -> Result<(u64, u64)> {
        // Network stats are typically in /proc/net/dev for the container's network namespace
        // For now, return zeros as network monitoring requires more complex setup
        Ok((0, 0))
    }
    
    fn get_block_io_usage(&self, cgroup_path: &Path) -> Result<(u64, u64)> {
        let io_stat_path = cgroup_path.join("io.stat");
        
        if !io_stat_path.exists() {
            return Ok((0, 0));
        }
        
        let content = fs::read_to_string(&io_stat_path)
            .context(format!("Failed to read {:?}", io_stat_path))?;
        
        let mut read_bytes = 0u64;
        let mut write_bytes = 0u64;
        
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                for chunk in parts[1..].chunks(2) {
                    if chunk.len() == 2 {
                        match chunk[0] {
                            "rbytes" => read_bytes += chunk[1].parse::<u64>().unwrap_or(0),
                            "wbytes" => write_bytes += chunk[1].parse::<u64>().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
            }
        }
        
        Ok((read_bytes, write_bytes))
    }
    
    fn get_system_memory(&self) -> Result<u64> {
        let meminfo = fs::read_to_string("/proc/meminfo")?;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let kb = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                return Ok(kb * 1024); // Convert KB to bytes
            }
        }
        Ok(0)
    }
}
