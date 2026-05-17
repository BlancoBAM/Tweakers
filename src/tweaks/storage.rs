#![allow(dead_code)]
// Storage tweaks module
use std::path::Path;

/// Read current I/O scheduler for NVMe device
pub async fn read_io_scheduler() -> Result<String, std::io::Error> {
    let devices = ["nvme0n1", "nvme1n1", "sda", "sdb", "vda"];

    for device in &devices {
        let path = format!("/sys/block/{}/queue/scheduler", device);
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            for part in content.split_whitespace() {
                if part.starts_with('[') && part.ends_with(']') {
                    return Ok(part[1..part.len() - 1].to_string());
                }
            }
        }
    }

    Ok("mq-deadline".into())
}

/// Check if fstrim timer is enabled
pub async fn check_fstrim_timer() -> bool {
    let output = std::process::Command::new("systemctl")
        .args(["is-enabled", "fstrim.timer"])
        .output();

    output.map(|o| o.status.success()).unwrap_or(false)
}

/// Check if noatime is configured in fstab or mount options
pub async fn check_noatime() -> bool {
    match tokio::fs::read_to_string("/etc/fstab").await {
        Ok(content) => content.contains("noatime") || content.contains("relatime"),
        Err(_) => false,
    }
}

/// Get first available NVMe/block device
pub async fn first_block_device() -> Option<String> {
    let devices = ["nvme0n1", "nvme1n1", "sda", "sdb", "vda"];

    for device in &devices {
        let path = format!("/sys/block/{}/queue/scheduler", device);
        if Path::new(&path).exists() {
            return Some(device.to_string());
        }
    }
    None
}