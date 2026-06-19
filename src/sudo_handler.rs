use std::process::Stdio;

use crate::tweaks;

/// Write a file using sudo with askpass
async fn write_file_with_sudo(
    askpass_path: &str,
    path: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create a temp file first
    let temp_path = format!("/tmp/tweakers_{}", std::process::id());
    tokio::fs::write(&temp_path, content).await?;

    // Move with sudo
    let output = tokio::process::Command::new("bash")
        .args(["-c", &format!("SUDO_ASKPASS={} sudo -A mv {} {} 2>&1", askpass_path, temp_path, path)])
        .env("SUDO_ASKPASS", askpass_path)
        .env_remove("DISPLAY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to write {}: {}", path, stderr.trim()).into());
    }

    // Set proper permissions
    let output = tokio::process::Command::new("bash")
        .args(["-c", &format!("SUDO_ASKPASS={} sudo -A chmod 644 {} 2>&1", askpass_path, path)])
        .env("SUDO_ASKPASS", askpass_path)
        .env_remove("DISPLAY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to chmod {}: {}", path, stderr.trim()).into());
    }

    Ok(())
}

/// Authenticate with sudo via askpass helper and apply pending changes
pub async fn authenticate_and_apply(
    askpass_path: &str,
    _password: &str,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::AppState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get current settings
    let state = state.lock().await;
    let settings = &state.tweaks;

    // Generate config files
    let sysctl_content = tweaks::generate_sysctl_config(settings);
    let i915_content = tweaks::generate_i915_config(settings);

    // Write sysctl config
    write_file_with_sudo(askpass_path, "/etc/sysctl.d/99-tweakers.conf", &sysctl_content).await?;

    // Apply sysctl immediately
    run_with_sudo(askpass_path, "sysctl -p /etc/sysctl.d/99-tweakers.conf 2>&1").await?;

    // Write i915 config if needed
    if !i915_content.is_empty() {
        write_file_with_sudo(
            askpass_path,
            "/etc/modprobe.d/tweakers-i915.conf",
            &i915_content,
        ).await?;
    }

    // Apply CPU governor
    if !settings.cpu_governor.is_empty() {
        let cmd = format!(
            "for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo {} > $cpu 2>/dev/null || true; done",
            settings.cpu_governor
        );
        run_with_sudo(askpass_path, &cmd).await.ok();
    }

    // Apply turbo boost setting
    let turbo_val = if settings.turbo_enabled { "0" } else { "1" };
    run_with_sudo(
        askpass_path,
        &format!("echo {} > /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null || true", turbo_val),
    ).await.ok();

    // Apply I/O scheduler
    run_with_sudo(
        askpass_path,
        &format!("echo {} > /sys/block/nvme0n1/queue/scheduler 2>/dev/null || true", settings.io_scheduler),
    ).await.ok();

    // Enable/disable fstrim timer
    let fstrim_cmd = if settings.trim_enabled {
        "systemctl enable --now fstrim.timer 2>&1"
    } else {
        "systemctl disable --now fstrim.timer 2>&1"
    };
    run_with_sudo(askpass_path, fstrim_cmd).await.ok();

    // WiFi power save
    if settings.wifi_powersave {
        run_with_sudo(askpass_path, "iwconfig wlo1 power on 2>/dev/null || true").await.ok();
    } else {
        run_with_sudo(askpass_path, "iwconfig wlo1 power off 2>/dev/null || true").await.ok();
    }

    // Audio power save
    let audio_val = if settings.audio_powersave { "1" } else { "0" };
    run_with_sudo(
        askpass_path,
        &format!("echo {} > /sys/module/snd_hda_intel/parameters/power_save 2>/dev/null || true", audio_val),
    ).await.ok();

    Ok(())
}

/// Run a command with sudo using askpass
async fn run_with_sudo(
    askpass_path: &str,
    command: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output = tokio::process::Command::new("bash")
        .args(["-c", &format!("SUDO_ASKPASS={} sudo -A bash -c '{}' 2>&1", askpass_path, command)])
        .env("SUDO_ASKPASS", askpass_path)
        .env_remove("DISPLAY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!("Command failed: {}\n{}", stderr.trim(), stdout.trim()).into())
    }
}

/// Reset all settings to system defaults
pub async fn reset_to_defaults(
    askpass_path: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Remove our config files
    run_with_sudo(askpass_path, "rm -f /etc/sysctl.d/99-tweakers.conf").await.ok();
    run_with_sudo(askpass_path, "rm -f /etc/modprobe.d/tweakers-i915.conf").await.ok();

    // Restore default sysctl
    run_with_sudo(askpass_path, "sysctl --system 2>&1").await?;

    // Reset CPU governor to schedutil
    run_with_sudo(
        askpass_path,
        "for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo schedutil > $cpu 2>/dev/null || true; done",
    ).await.ok();

    // Enable turbo boost
    run_with_sudo(askpass_path, "echo 0 > /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null || true").await.ok();

    // Reset I/O scheduler
    run_with_sudo(askpass_path, "echo mq-deadline > /sys/block/nvme0n1/queue/scheduler 2>/dev/null || true").await.ok();

    Ok(())
}

/// Clean system files and caches
pub async fn clean_system(
    askpass_path: &str,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::AppState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        run_with_sudo(askpass_path, "apt-get clean 2>&1").await?;
        run_with_sudo(askpass_path, "apt-get autoremove --purge -y 2>&1").await.ok();
    }

    // Clean thumbnail cache
    if thumb {
        if let Some(home) = dirs::home_dir() {
            let thumb_path = home.join(".cache/thumbnails");
            if thumb_path.exists() {
                let cmd = format!("rm -rf {}/thumb-* 2>/dev/null || true", thumb_path.display());
                run_with_sudo(askpass_path, &cmd).await.ok();
            }
        }
    }

    // Vacuum journal logs
    if journal {
        run_with_sudo(askpass_path, "journalctl --vacuum-size=100M 2>&1").await.ok();
    }

    // Clean temp files safely
    if temp {
        run_with_sudo(askpass_path, "find /tmp -mindepth 1 -delete 2>/dev/null || true").await.ok();
        run_with_sudo(askpass_path, "find /var/tmp -mindepth 1 -delete 2>/dev/null || true").await.ok();
    }

    // Remove orphaned package configs
    if orphans {
        run_with_sudo(
            askpass_path,
            "dpkg -l | grep ^rc | awk '{print $2}' | xargs -r dpkg --purge 2>/dev/null || true",
        ).await.ok();
    }

    Ok(())
}