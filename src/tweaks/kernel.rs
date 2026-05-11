use tokio::fs;

/// Read Transparent Huge Pages setting
pub async fn read_thp() -> Result<String, std::io::Error> {
    let content = fs::read_to_string("/sys/kernel/mm/transparent_hugepage/enabled").await?;
    // Format: "always [madvise] never"
    for part in content.split_whitespace() {
        if part.starts_with('[') && part.ends_with(']') {
            return Ok(part[1..part.len() - 1].to_string());
        }
    }
    Ok("madvise".into())
}

/// Read watchdog setting (1 = enabled, 0 = disabled)
pub async fn read_watchdog() -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    read_sysctl_value("kernel.watchdog").await
}

/// Read NMI watchdog setting
pub async fn read_nmi_watchdog() -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    read_sysctl_value("kernel.nmi_watchdog").await
}

/// Read sysctl value from /proc/sys
pub async fn read_sysctl_value(key: &str) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let path = format!("/proc/sys/{}", key.replace('.', "/"));
    let content = fs::read_to_string(&path).await?;
    Ok(content.trim().parse().unwrap_or(0))
}

/// Get kernel version
pub fn kernel_version() -> Option<String> {
    if let Ok(content) = std::fs::read_to_string("/proc/version") {
        // Extract just the version string
        let parts: Vec<&str> = content.splitn(4, ' ').collect();
        if parts.len() >= 3 {
            return Some(parts[2].to_string());
        }
        Some(content.trim().to_string())
    } else {
        None
    }
}

/// Get system uptime
pub fn uptime() -> Option<f64> {
    if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if !parts.is_empty() {
            return parts[0].parse().ok();
        }
    }
    None
}