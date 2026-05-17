#![allow(dead_code)]
// Power management tweaks module
use std::path::Path;

/// Check WiFi power save mode
pub async fn check_wifi_powersave() -> bool {
    // Try iw first
    let interfaces = ["wlan0", "wlo1", "wifi0"];
    for iface in &interfaces {
        let output = std::process::Command::new("iw")
            .args(["dev", iface, "get", "power_save"])
            .output();

        if let Ok(o) = output {
            if o.status.success() {
                let stdout = String::from_utf8_lossy(&o.stdout);
                return stdout.trim().ends_with("on");
            }
        }
    }

    // Fallback to iwconfig
    for iface in interfaces {
        let output = std::process::Command::new("iwconfig")
            .arg(iface)
            .output();

        if let Ok(o) = output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.contains("Power Management:on") {
                return true;
            } else if stdout.contains("Power Management:off") {
                return false;
            }
        }
    }

    false
}

/// Get wireless interface name
pub fn wireless_interface() -> Option<String> {
    let output = std::process::Command::new("ip")
        .args(["link", "show"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("wlan") || line.contains("wlo") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let name = parts[1].trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// Check audio power save
pub async fn check_audio_powersave() -> bool {
    let paths = [
        "/sys/module/snd_hda_intel/parameters/power_save",
        "/sys/module/snd_ac97_codec/parameters/power_save",
    ];

    for path in &paths {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            return content.trim() != "0";
        }
    }
    false
}

/// Get PCIe ASPM (Active State Power Management) status
pub async fn read_pcie_aspm() -> Result<String, std::io::Error> {
    let content = tokio::fs::read_to_string("/sys/module/pcie_aspm/parameters/policy").await?;
    Ok(content.trim().to_string())
}

/// Get battery info if available
pub fn battery_info() -> Option<(f64, Option<String>)> {
    let paths = ["/sys/class/power_supply/BAT0", "/sys/class/power_supply/BAT1"];

    for path in &paths {
        if Path::new(&format!("{}/capacity", path)).exists() {
            if let Ok(capacity) = std::fs::read_to_string(format!("{}/capacity", path)) {
                if let Ok(pct) = capacity.trim().parse::<f64>() {
                    let status = std::fs::read_to_string(format!("{}/status", path))
                        .map(|s| s.trim().to_string())
                        .ok();
                    return Some((pct, status));
                }
            }
        }
    }
    None
}