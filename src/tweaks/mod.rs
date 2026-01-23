pub mod cpu;
pub mod memory;
pub mod storage;
pub mod gpu;
pub mod power;
pub mod network;
pub mod kernel;

use serde::{Deserialize, Serialize};
use std::process::Command;
use tokio::fs;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct TweakSettings {
    // CPU
    pub cpu_governor: String, // powersave, schedutil, performance
    pub turbo_enabled: bool,
    
    // Memory
    pub swappiness: u32,
    pub vfs_cache_pressure: u32,
    pub zram_enabled: bool,
    pub dirty_ratio: u32,
    pub dirty_background_ratio: u32,
    
    // Storage
    pub io_scheduler: String, // none, mq-deadline, kyber
    pub trim_enabled: bool,
    pub noatime: bool,
    
    // GPU (Intel)
    pub intel_guc: bool,
    pub intel_psr: bool,
    pub intel_fbc: bool,
    pub intel_rc6: bool,
    
    // Power
    pub wifi_powersave: bool,
    pub audio_powersave: bool,
    pub pcie_aspm: String,

    // Network
    pub tcp_fastopen: u32,
    pub tcp_low_latency: bool,
    pub bbr_enabled: bool,

    // Kernel
    pub watchdog_enabled: bool,
    pub transparent_hugepages: String, // always, madvise, never
}

/// Load current system settings
pub async fn load_current_settings() -> TweakSettings {
    TweakSettings {
        cpu_governor: read_cpu_governor().await.unwrap_or_else(|_| "schedutil".into()),
        turbo_enabled: !read_turbo_disabled().await.unwrap_or(false),
        swappiness: read_sysctl("vm.swappiness").await.unwrap_or(60),
        vfs_cache_pressure: read_sysctl("vm.vfs_cache_pressure").await.unwrap_or(100),
        zram_enabled: check_zram_enabled().await,
        dirty_ratio: read_sysctl("vm.dirty_ratio").await.unwrap_or(20),
        dirty_background_ratio: read_sysctl("vm.dirty_background_ratio").await.unwrap_or(10),
        io_scheduler: read_io_scheduler().await.unwrap_or_else(|_| "mq-deadline".into()),
        trim_enabled: check_fstrim_timer().await,
        noatime: check_noatime().await,
        intel_guc: false, // Requires parsing modprobe conf
        intel_psr: false,
        intel_fbc: false,
        intel_rc6: false,
        wifi_powersave: check_wifi_powersave().await,
        audio_powersave: check_audio_powersave().await,
        pcie_aspm: "default".into(),
        tcp_fastopen: read_sysctl("net.ipv4.tcp_fastopen").await.unwrap_or(3),
        tcp_low_latency: read_sysctl("net.ipv4.tcp_low_latency").await.unwrap_or(0) == 1,
        bbr_enabled: check_bbr().await,
        watchdog_enabled: read_sysctl("kernel.watchdog").await.unwrap_or(1) == 1,
        transparent_hugepages: read_thp().await.unwrap_or_else(|_| "madvise".into()),
    }
}

/// Read CPU governor from sysfs
async fn read_cpu_governor() -> Result<String, std::io::Error> {
    fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .await
        .map(|s| s.trim().to_string())
}

/// Check if turbo is disabled
async fn read_turbo_disabled() -> Result<bool, std::io::Error> {
    let content = fs::read_to_string("/sys/devices/system/cpu/intel_pstate/no_turbo").await?;
    Ok(content.trim() == "1")
}

/// Read a sysctl value
async fn read_sysctl(key: &str) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let path = format!("/proc/sys/{}", key.replace('.', "/"));
    let content = fs::read_to_string(&path).await?;
    Ok(content.trim().parse()?)
}

/// Check if ZRAM is enabled
async fn check_zram_enabled() -> bool {
    fs::metadata("/dev/zram0").await.is_ok()
}

/// Read I/O scheduler for nvme0n1
async fn read_io_scheduler() -> Result<String, std::io::Error> {
    let content = fs::read_to_string("/sys/block/nvme0n1/queue/scheduler").await?;
    // Format: "[none] mq-deadline kyber" - brackets indicate active
    for part in content.split_whitespace() {
        if part.starts_with('[') && part.ends_with(']') {
            return Ok(part[1..part.len()-1].to_string());
        }
    }
    Ok("mq-deadline".into())
}

/// Check if fstrim.timer is enabled
async fn check_fstrim_timer() -> bool {
    Command::new("systemctl")
        .args(["is-enabled", "fstrim.timer"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if noatime is set in fstab
async fn check_noatime() -> bool {
    fs::read_to_string("/etc/fstab")
        .await
        .map(|s| s.contains("noatime"))
        .unwrap_or(false)
}

/// Check WiFi power save
async fn check_wifi_powersave() -> bool {
    Command::new("iwconfig")
        .arg("wlo1")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Power Management:on"))
        .unwrap_or(false)
}

/// Check audio power save
async fn check_audio_powersave() -> bool {
    fs::read_to_string("/sys/module/snd_hda_intel/parameters/power_save")
        .await
        .map(|s| s.trim() != "0")
        .unwrap_or(false)
}

/// Generate sysctl configuration file content
pub fn generate_sysctl_config(settings: &TweakSettings) -> String {
    let mut config = format!(
        r#"# Tweakers - System Optimizations
# Generated automatically - do not edit manually

# Memory Management
vm.swappiness = {}
vm.vfs_cache_pressure = {}
vm.dirty_ratio = {}
vm.dirty_background_ratio = {}
vm.min_free_kbytes = 65536

# Network Optimizations
net.core.rmem_default = 262144
net.core.rmem_max = 16777216
net.core.wmem_default = 262144
net.core.wmem_max = 16777216
net.core.netdev_max_backlog = 5000
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65536
net.ipv4.tcp_fin_timeout = 10
"#,
        settings.swappiness,
        settings.vfs_cache_pressure,
        settings.dirty_ratio,
        settings.dirty_background_ratio,
    );

    // Network Optimizations
    config.push_str(&format!(
        r#"
# Custom Network Optimizations
net.ipv4.tcp_fastopen = {}
net.ipv4.tcp_low_latency = {}
"#,
        settings.tcp_fastopen,
        if settings.tcp_low_latency { 1 } else { 0 },
    ));

    if settings.bbr_enabled {
        config.push_str(
            r#"net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr
"#,
        );
    }

    // Kernel Optimizations
    config.push_str(&format!(
        r#"
# Kernel Optimizations
kernel.watchdog = {}
kernel.nmi_watchdog = {}
"#,
        if settings.watchdog_enabled { 1 } else { 0 },
        if settings.watchdog_enabled { 1 } else { 0 },
    ));

    config
}

/// Check if BBR is enabled
async fn check_bbr() -> bool {
    if let Ok(cc) = fs::read_to_string("/proc/sys/net/ipv4/tcp_congestion_control").await {
        return cc.trim() == "bbr";
    }
    false
}

/// Read Transparent Huge Pages setting
async fn read_thp() -> Result<String, std::io::Error> {
    let content = fs::read_to_string("/sys/kernel/mm/transparent_hugepage/enabled").await?;
    // Format: "always [madvise] never"
    for part in content.split_whitespace() {
        if part.starts_with('[') && part.ends_with(']') {
            return Ok(part[1..part.len() - 1].to_string());
        }
    }
    Ok("madvise".into())
}


/// Generate i915 modprobe config  
pub fn generate_i915_config(settings: &TweakSettings) -> String {
    let mut options = Vec::new();
    
    if settings.intel_guc {
        options.push("enable_guc=2");
    }
    if settings.intel_fbc {
        options.push("enable_fbc=1");
    }
    if settings.intel_psr {
        options.push("enable_psr=1");
    }
    if settings.intel_rc6 {
        options.push("enable_rc6=1");
    }
    
    if options.is_empty() {
        String::new()
    } else {
        format!("options i915 {}", options.join(" "))
    }
}
