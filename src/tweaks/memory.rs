/// Read swappiness value (0-100, default 60)
pub async fn read_swappiness() -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let value = read_sysctl_value("vm.swappiness").await?;
    Ok(value)
}

/// Read VFS cache pressure (0-200, default 100)
pub async fn read_vfs_cache_pressure() -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let value = read_sysctl_value("vm.vfs_cache_pressure").await?;
    Ok(value)
}

/// Read dirty page ratio (0-100, default 20)
pub async fn read_dirty_ratio() -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let value = read_sysctl_value("vm.dirty_ratio").await?;
    Ok(value)
}

/// Read dirty background ratio (0-100, default 10)
pub async fn read_dirty_background_ratio() -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let value = read_sysctl_value("vm.dirty_background_ratio").await?;
    Ok(value)
}

/// Check if ZRAM is enabled
pub async fn check_zram_enabled() -> bool {
    tokio::fs::metadata("/dev/zram0").await.is_ok() || tokio::fs::metadata("/dev/zram1").await.is_ok()
}

/// Read a sysctl value from /proc/sys
pub async fn read_sysctl_value(key: &str) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let path = format!("/proc/sys/{}", key.replace('.', "/"));
    let content = tokio::fs::read_to_string(&path).await?;
    Ok(content.trim().parse().unwrap_or(0))
}

/// Get total and available memory in MiB
pub async fn get_memory_info() -> Result<(u64, u64), Box<dyn std::error::Error + Send + Sync>> {
    let content = tokio::fs::read_to_string("/proc/meminfo").await?;
    let mut total_kb: u64 = 0;
    let mut avail_kb: u64 = 0;

    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            avail_kb = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        }
    }

    Ok((total_kb / 1024, avail_kb / 1024))
}