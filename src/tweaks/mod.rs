#![allow(dead_code)]
pub mod cpu;
pub mod memory;
pub mod storage;
pub mod gpu;
pub mod power;
pub mod network;
pub mod kernel;

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct TweakSettings {
    // CPU
    pub cpu_governor: String,          // powersave, schedutil, performance
    pub turbo_enabled: bool,

    // Memory
    pub swappiness: u32,
    pub vfs_cache_pressure: u32,
    pub dirty_ratio: u32,
    pub dirty_background_ratio: u32,
    pub zram_enabled: bool,

    // Storage
    pub io_scheduler: String,          // none, mq-deadline, kyber
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

/// Load current system settings by reading from sysfs/proc/sys
pub async fn load_current_settings() -> TweakSettings {
    // Use submodule functions where available
    let cpu_governor = cpu::read_governor()
        .await
        .unwrap_or_else(|_| "schedutil".into());

    let turbo_enabled = !cpu::read_turbo_disabled()
        .await
        .unwrap_or(false);

    let swappiness = memory::read_swappiness()
        .await
        .unwrap_or(60);

    let vfs_cache_pressure = memory::read_vfs_cache_pressure()
        .await
        .unwrap_or(100);

    let dirty_ratio = memory::read_dirty_ratio()
        .await
        .unwrap_or(20);

    let dirty_background_ratio = memory::read_dirty_background_ratio()
        .await
        .unwrap_or(10);

    let zram_enabled = memory::check_zram_enabled().await;

    let io_scheduler = storage::read_io_scheduler()
        .await
        .unwrap_or_else(|_| "mq-deadline".into());

    let trim_enabled = storage::check_fstrim_timer().await;
    let noatime = storage::check_noatime().await;

    let intel_guc = gpu::read_intel_guc().await;
    let intel_psr = gpu::read_intel_psr().await;
    let intel_fbc = gpu::read_intel_fbc().await;
    let intel_rc6 = gpu::read_intel_rc6().await;

    let wifi_powersave = power::check_wifi_powersave().await;
    let audio_powersave = power::check_audio_powersave().await;
    let pcie_aspm = power::read_pcie_aspm().await.unwrap_or_else(|_| "default".into());

    let tcp_fastopen = network::read_tcp_fastopen()
        .await
        .unwrap_or(3);

    let tcp_low_latency = network::read_tcp_low_latency()
        .await
        .unwrap_or(false);

    let bbr_enabled = network::check_bbr().await;

    let watchdog_enabled = kernel::read_watchdog()
        .await
        .map(|v| v == 1)
        .unwrap_or(true);

    let transparent_hugepages = kernel::read_thp()
        .await
        .unwrap_or_else(|_| "madvise".into());

    TweakSettings {
        cpu_governor,
        turbo_enabled,
        swappiness,
        vfs_cache_pressure,
        dirty_ratio,
        dirty_background_ratio,
        zram_enabled,
        io_scheduler,
        trim_enabled,
        noatime,
        intel_guc,
        intel_psr,
        intel_fbc,
        intel_rc6,
        wifi_powersave,
        audio_powersave,
        pcie_aspm,
        tcp_fastopen,
        tcp_low_latency,
        bbr_enabled,
        watchdog_enabled,
        transparent_hugepages,
    }
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

    // Custom Network Optimizations
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

/// Reapply all current settings (used after reset)
pub async fn reapply_settings(settings: &TweakSettings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Apply sysctl
    let sysctl_content = generate_sysctl_config(settings);
    tokio::fs::write("/etc/sysctl.d/99-tweakers.conf", &sysctl_content).await?;

    let output = Command::new("sysctl")
        .args(["-p", "/etc/sysctl.d/99-tweakers.conf"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("sysctl apply failed: {}", stderr).into());
    }

    // Apply i915 config (requires modprobe reload)
    let i915_content = generate_i915_config(settings);
    if !i915_content.is_empty() {
        tokio::fs::write("/etc/modprobe.d/tweakers-i915.conf", &i915_content).await?;

        Command::new("modprobe")
            .args(["-r", "i915"])
            .status()
            .ok();
        Command::new("modprobe")
            .arg("i915")
            .status()
            .ok();
    }

    Ok(())
}