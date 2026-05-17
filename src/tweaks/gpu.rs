#![allow(dead_code)]
use std::path::Path;
use tokio::fs;

/// Check if Intel GPU is present
pub fn intel_gpu_present() -> bool {
    Path::new("/sys/module/i915").exists()
}

/// Read Intel GuC firmware loading status
pub async fn read_intel_guc() -> bool {
    if let Ok(content) = fs::read_to_string("/sys/module/i915/parameters/enable_guc").await {
        let trimmed = content.trim();
        trimmed == "2" || trimmed == "3" || trimmed.to_lowercase() == "y"
    } else {
        false
    }
}

/// Read Intel PSR (Panel Self Refresh) status
pub async fn read_intel_psr() -> bool {
    if let Ok(content) = fs::read_to_string("/sys/module/i915/parameters/enable_psr").await {
        let trimmed = content.trim();
        trimmed == "1" || trimmed.to_lowercase() == "y"
    } else {
        false
    }
}

/// Read Intel FBC (Framebuffer Compression) status
pub async fn read_intel_fbc() -> bool {
    if let Ok(content) = fs::read_to_string("/sys/module/i915/parameters/enable_fbc").await {
        let trimmed = content.trim();
        trimmed == "1" || trimmed.to_lowercase() == "y"
    } else {
        false
    }
}

/// Read Intel RC6 (Runtime D3) status
pub async fn read_intel_rc6() -> bool {
    if let Ok(content) = fs::read_to_string("/sys/module/i915/parameters/enable_rc6").await {
        let trimmed = content.trim();
        trimmed != "0"
    } else {
        false
    }
}

/// Get Intel GPU frequency info
pub async fn intel_gpu_freq_info() -> Option<String> {
    let path = "/sys/kernel/debug/dri/0/i915_frequency_info";
    if Path::new(path).exists() {
        fs::read_to_string(path).await.ok()
    } else {
        None
    }
}

/// Get Intel GPU memory info
pub async fn intel_gpu_memory_info() -> Option<String> {
    let path = "/sys/kernel/debug/dri/0/i915_gem_objects";
    if Path::new(path).exists() {
        fs::read_to_string(path).await.ok()
    } else {
        None
    }
}