use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CgroupManager {
    cgroup_root: PathBuf,
}

impl CgroupManager {
    pub fn new() -> Result<Self> {
        let cgroup_root = PathBuf::from("/sys/fs/cgroup");
        
        if !cgroup_root.exists() {
            anyhow::bail!("cgroupv2 not found at /sys/fs/cgroup");
        }
        
        Ok(Self { cgroup_root })
    }
    
    pub fn find_container_cgroup(&self, container_name_or_id: &str) -> Result<PathBuf> {
        // First try to get container ID from Docker
        let container_id = self.resolve_container_id(container_name_or_id)?;
        
        // Look for the container in Docker's cgroup hierarchy
        let system_slice_path = self.cgroup_root.join("system.slice");
        
        if system_slice_path.exists() {
            // Look for container-specific cgroup directly in system.slice
            let container_path = system_slice_path.join(format!("docker-{}.scope", container_id));
            if container_path.exists() {
                return Ok(container_path);
            }
            
            // Also try with short container ID (first 12 chars)
            let short_id = &container_id[..12.min(container_id.len())];
            let container_path_short = system_slice_path.join(format!("docker-{}.scope", short_id));
            if container_path_short.exists() {
                return Ok(container_path_short);
            }
            
            // Search recursively in system.slice
            if let Ok(path) = self.search_for_container(&system_slice_path, &container_id) {
                return Ok(path);
            }
        }
        
        // Alternative: search in user.slice for rootless Docker
        let user_slice_path = self.cgroup_root.join("user.slice");
        if user_slice_path.exists() {
            if let Ok(path) = self.search_for_container(&user_slice_path, &container_id) {
                return Ok(path);
            }
        }
        
        anyhow::bail!("Container {} not found in cgroup hierarchy", container_name_or_id)
    }
    
    fn resolve_container_id(&self, name_or_id: &str) -> Result<String> {
        // Try using docker inspect to get full container ID
        let output = std::process::Command::new("docker")
            .args(["inspect", "--format", "{{.Id}}", name_or_id])
            .output()
            .context("Failed to run docker inspect")?;
            
        if output.status.success() {
            let id = String::from_utf8(output.stdout)?
                .trim()
                .to_string();
            return Ok(id);
        }
        
        // If docker command fails, assume it's already a container ID
        Ok(name_or_id.to_string())
    }
    
    fn search_for_container(&self, base_path: &Path, container_id: &str) -> Result<PathBuf> {
        let entries = fs::read_dir(base_path)
            .context(format!("Failed to read directory: {:?}", base_path))?;
            
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                    
                // Check if this directory contains our container ID
                if name.contains(container_id) || name.contains(&container_id[..12]) {
                    return Ok(path);
                }
                
                // Recursively search subdirectories
                if let Ok(found) = self.search_for_container(&path, container_id) {
                    return Ok(found);
                }
            }
        }
        
        anyhow::bail!("Container not found in {}", base_path.display())
    }
}
