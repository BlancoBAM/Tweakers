#![allow(dead_code)]
// Network tweaks module

/// Check if BBR congestion control is enabled
pub async fn check_bbr() -> bool {
    if let Ok(cc) = tokio::fs::read_to_string("/proc/sys/net/ipv4/tcp_congestion_control").await {
        return cc.trim() == "bbr";
    }
    false
}

/// Read TCP Fast Open value (0=off, 1=client, 2=server, 3=both)
pub async fn read_tcp_fastopen() -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let value = read_sysctl_value("net.ipv4.tcp_fastopen").await?;
    Ok(value)
}

/// Read TCP low latency setting
pub async fn read_tcp_low_latency() -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let value = read_sysctl_value("net.ipv4.tcp_low_latency").await?;
    Ok(value == 1)
}

/// Get current TCP congestion control algorithm
pub async fn tcp_congestion_control() -> Result<String, std::io::Error> {
    tokio::fs::read_to_string("/proc/sys/net/ipv4/tcp_congestion_control")
        .await
        .map(|s| s.trim().to_string())
}

/// List available TCP congestion control algorithms
pub fn available_congestion_controls() -> Vec<String> {
    if let Ok(output) = std::process::Command::new("sysctl")
        .args(["net.ipv4.tcp_available_congestion_control"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(value) = stdout.split('=').nth(1) {
                return value.split_whitespace().map(|s| s.to_string()).collect();
            }
        }
    }
    vec!["cubic".into(), "bbr".into()]
}

/// Read sysctl value from /proc/sys
pub async fn read_sysctl_value(key: &str) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let path = format!("/proc/sys/{}", key.replace('.', "/"));
    let content = tokio::fs::read_to_string(&path).await?;
    Ok(content.trim().parse().unwrap_or(0))
}

/// Get network interface statistics
pub async fn network_stats(iface: &str) -> Option<(u64, u64)> {
    let path = format!("/sys/class/net/{}/statistics", iface);
    let rx_bytes = tokio::fs::read_to_string(format!("{}/rx_bytes", path)).await.ok()?;
    let tx_bytes = tokio::fs::read_to_string(format!("{}/tx_bytes", path)).await.ok()?;

    Some((
        rx_bytes.trim().parse().unwrap_or(0),
        tx_bytes.trim().parse().unwrap_or(0),
    ))
}