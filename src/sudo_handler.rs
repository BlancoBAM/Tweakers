use std::process::{Command, Stdio};
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::fs;

use crate::AppState;
use crate::tweaks;

/// Authenticate with sudo and apply pending changes
pub async fn authenticate_and_apply(
    password: &str,
    state: &Arc<Mutex<AppState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Verify password first with a simple command
    verify_password(password).await?;
    
    // Get current settings
    let state = state.lock().await;
    let settings = &state.tweaks;
    
    // Generate config files
    let sysctl_content = tweaks::generate_sysctl_config(settings);
    let i915_content = tweaks::generate_i915_config(settings);
    
    // Write sysctl config
    write_file_with_sudo(
        password,
        "/etc/sysctl.d/99-tweakers.conf",
        &sysctl_content,
    ).await?;
    
    // Apply sysctl immediately
    run_with_sudo(password, "sysctl -p /etc/sysctl.d/99-tweakers.conf").await?;
    
    // Write i915 config if needed
    if !i915_content.is_empty() {
        write_file_with_sudo(
            password,
            "/etc/modprobe.d/tweakers-i915.conf",
            &i915_content,
        ).await?;
    }
    
    // Apply CPU governor
    if !settings.cpu_governor.is_empty() {
        let cmd = format!(
            "for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo {} > $cpu; done",
            settings.cpu_governor
        );
        run_with_sudo(password, &cmd).await?;
    }
    
    // Apply turbo boost setting
    let turbo_val = if settings.turbo_enabled { "0" } else { "1" };
    run_with_sudo(
        password,
        &format!("echo {} > /sys/devices/system/cpu/intel_pstate/no_turbo", turbo_val),
    ).await.ok(); // May fail if intel_pstate not available
    
    // Apply I/O scheduler
    run_with_sudo(
        password,
        &format!("echo {} > /sys/block/nvme0n1/queue/scheduler", settings.io_scheduler),
    ).await.ok();
    
    // Enable/disable fstrim timer
    let fstrim_cmd = if settings.trim_enabled {
        "systemctl enable --now fstrim.timer"
    } else {
        "systemctl disable --now fstrim.timer"
    };
    run_with_sudo(password, fstrim_cmd).await?;
    
    // WiFi power save
    if settings.wifi_powersave {
        run_with_sudo(password, "iwconfig wlo1 power on").await.ok();
    } else {
        run_with_sudo(password, "iwconfig wlo1 power off").await.ok();
    }
    
    // Audio power save
    let audio_val = if settings.audio_powersave { "1" } else { "0" };
    run_with_sudo(
        password,
        &format!("echo {} > /sys/module/snd_hda_intel/parameters/power_save", audio_val),
    ).await.ok();
    
    Ok(())
}

/// Verify sudo password is correct
async fn verify_password(password: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut child = Command::new("sudo")
        .args(["-S", "-v"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(password.as_bytes())?;
        stdin.write_all(b"\n")?;
    }
    
    let output = child.wait_with_output()?;
    
    if output.status.success() {
        Ok(())
    } else {
        Err("Incorrect password".into())
    }
}

/// Run a command with sudo
async fn run_with_sudo(
    password: &str,
    command: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut child = Command::new("sudo")
        .args(["-S", "bash", "-c", command])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(password.as_bytes())?;
        stdin.write_all(b"\n")?;
    }
    
    let output = child.wait_with_output()?;
    
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Command failed: {}", stderr).into())
    }
}

/// Write a file using sudo
async fn write_file_with_sudo(
    password: &str,
    path: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create a temp file first
    let temp_path = format!("/tmp/tweakers_{}", std::process::id());
    fs::write(&temp_path, content).await?;
    
    // Move with sudo
    run_with_sudo(password, &format!("mv {} {}", temp_path, path)).await?;
    
    // Set proper permissions
    run_with_sudo(password, &format!("chmod 644 {}", path)).await?;
    
    Ok(())
}

/// Reset all settings to system defaults
pub async fn reset_to_defaults(
    password: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Verify password first
    verify_password(password).await?;
    
    // Remove our config files
    run_with_sudo(password, "rm -f /etc/sysctl.d/99-tweakers.conf").await.ok();
    run_with_sudo(password, "rm -f /etc/modprobe.d/tweakers-i915.conf").await.ok();
    
    // Restore default sysctl
    run_with_sudo(password, "sysctl --system").await?;
    
    // Reset CPU governor to schedutil
    run_with_sudo(
        password,
        "for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo schedutil > $cpu; done",
    ).await.ok();
    
    // Enable turbo boost
    run_with_sudo(password, "echo 0 > /sys/devices/system/cpu/intel_pstate/no_turbo").await.ok();
    
    // Reset I/O scheduler
    run_with_sudo(password, "echo mq-deadline > /sys/block/nvme0n1/queue/scheduler").await.ok();
    
    Ok(())
}

/// Clean system files and caches
pub async fn clean_system(
    password: &str,
    state: &Arc<Mutex<AppState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Verify password first
    verify_password(password).await?;
    
    let (apt, thumb, journal, temp, orphans) = {
        let s = state.lock().await;
        (
            s.cleaner.apt_cache_selected,
            s.cleaner.thumbnail_selected,
            s.cleaner.journal_selected,
            s.cleaner.temp_selected,
            s.cleaner.orphan_selected,
        )
    };
    
    // Clean APT cache
    if apt {
        println!("Cleaning APT cache...");
        run_with_sudo(password, "apt-get clean").await?;
        run_with_sudo(password, "apt-get autoremove --purge -y").await.ok();
    }
    
    // Clean thumbnail cache
    if thumb {
        if let Some(home) = dirs::home_dir() {
            let thumb_path = home.join(".cache/thumbnails");
            if thumb_path.exists() {
                println!("Cleaning thumbnail cache...");
                let cmd = format!("rm -rf {}/*", thumb_path.display());
                std::process::Command::new("bash")
                    .args(["-c", &cmd])
                    .status()
                    .ok();
            }
        }
    }
    
    // Vacuum journal logs
    if journal {
        println!("Vacuuming journal logs...");
        run_with_sudo(password, "journalctl --vacuum-size=100M").await.ok();
    }
    
    // Clean temp files
    if temp {
        println!("Cleaning temp files...");
        run_with_sudo(password, "rm -rf /tmp/* 2>/dev/null || true").await.ok();
        run_with_sudo(password, "rm -rf /var/tmp/* 2>/dev/null || true").await.ok();
    }
    
    // Remove orphaned package configs
    if orphans {
        println!("Removing orphaned package configs...");
        run_with_sudo(
            password, 
            "dpkg -l | grep ^rc | awk '{print $2}' | xargs -r dpkg --purge 2>/dev/null || true"
        ).await.ok();
    }
    
    println!("System cleaning complete!");
    Ok(())
}


