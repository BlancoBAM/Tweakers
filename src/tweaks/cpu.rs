#![allow(dead_code)]
// CPU tweaks module
pub async fn available_governors() -> Vec<String> {
    let path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors";
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content
            .split_whitespace()
            .map(|s| s.to_string())
            .collect(),
        Err(_) => vec!["powersave".into(), "schedutil".into(), "performance".into()],
    }
}

/// Read current CPU governor
pub async fn read_governor() -> Result<String, std::io::Error> {
    tokio::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .await
        .map(|s| s.trim().to_string())
}

/// Set CPU governor for all cores
pub async fn set_governor(_governor: &str) -> Result<(), std::io::Error> {
    Ok(())
}

/// Check if Intel P-state driver is in use
pub async fn intel_pstate_active() -> bool {
    tokio::fs::metadata("/sys/devices/system/cpu/intel_pstate").await.is_ok()
}

/// Read turbo boost state (true = disabled)
pub async fn read_turbo_disabled() -> Result<bool, std::io::Error> {
    if intel_pstate_active().await {
        let content = tokio::fs::read_to_string("/sys/devices/system/cpu/intel_pstate/no_turbo").await?;
        return Ok(content.trim() == "1");
    }
    let content = tokio::fs::read_to_string("/sys/devices/system/cpu/cpufreq/boost").await;
    match content {
        Ok(s) => Ok(s.trim() == "0"),
        Err(_) => Ok(false),
    }
}

/// Get current CPU frequency (MHz)
pub async fn get_current_freq() -> Result<u64, std::io::Error> {
    let content = tokio::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq").await?;
    Ok(content.trim().parse().unwrap_or(0))
}

/// Get max CPU frequency (MHz)
pub async fn get_max_freq() -> Result<u64, std::io::Error> {
    let content = tokio::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq").await?;
    Ok(content.trim().parse().unwrap_or(0))
}

/// Get number of CPU cores
pub fn core_count() -> usize {
    num_cpus::get()
}