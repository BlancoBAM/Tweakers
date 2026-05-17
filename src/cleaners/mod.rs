#![allow(dead_code)]
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Clone, Default, Debug, Copy)]
pub struct CleanerState {
    pub apt_cache_mb: f64,
    pub thumbnail_cache_mb: f64,
    pub journal_mb: f64,
    pub temp_mb: f64,
    pub orphan_count: u32,

    pub apt_cache_selected: bool,
    pub thumbnail_selected: bool,
    pub journal_selected: bool,
    pub temp_selected: bool,
    pub orphan_selected: bool,
}

/// Scan all cleanable areas
pub async fn scan_all() -> CleanerState {
    let apt = scan_apt_cache().await;
    let thumbnail = scan_thumbnails().await;
    let journal = scan_journal().await;
    let temp = scan_temp().await;
    let orphan = scan_orphans().await;

    CleanerState {
        apt_cache_mb: apt,
        thumbnail_cache_mb: thumbnail,
        journal_mb: journal,
        temp_mb: temp,
        orphan_count: orphan,
        apt_cache_selected: true,
        thumbnail_selected: true,
        journal_selected: false,
        temp_selected: true,
        orphan_selected: false,
    }
}

/// Get APT cache size in MB
async fn scan_apt_cache() -> f64 {
    let path = PathBuf::from("/var/cache/apt/archives");
    calculate_dir_size(&path).await as f64 / 1_000_000.0
}

/// Get thumbnail cache size in MB
async fn scan_thumbnails() -> f64 {
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".cache/thumbnails");
        calculate_dir_size(&path).await as f64 / 1_000_000.0
    } else {
        0.0
    }
}

/// Get systemd journal size in MB
async fn scan_journal() -> f64 {
    let output = Command::new("journalctl")
        .args(["--disk-usage"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse output like "Archived and active journals take up 32.0M in the file system."
        for word in stdout.split_whitespace() {
            if word.ends_with('M') {
                if let Ok(num) = word[..word.len() - 1].parse::<f64>() {
                    return num;
                }
            } else if word.ends_with('G') {
                if let Ok(num) = word[..word.len() - 1].parse::<f64>() {
                    return num * 1000.0;
                }
            }
        }
    }
    0.0
}

/// Get temp directory size in MB
async fn scan_temp() -> f64 {
    let tmp = calculate_dir_size(&PathBuf::from("/tmp")).await;
    let var_tmp = calculate_dir_size(&PathBuf::from("/var/tmp")).await;
    (tmp + var_tmp) as f64 / 1_000_000.0
}

/// Count orphaned packages (residcfg packages)
async fn scan_orphans() -> u32 {
    let output = Command::new("dpkg")
        .args(["-l"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .filter(|line| line.starts_with("rc "))
            .count() as u32
    } else {
        0
    }
}

/// Calculate directory size recursively in bytes
async fn calculate_dir_size(path: &PathBuf) -> u64 {
    if !path.exists() {
        return 0;
    }

    let mut total: u64 = 0;
    // Only go 3 levels deep for safety and performance
    for entry in WalkDir::new(path)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

/// Clean APT cache (requires sudo)
pub fn clean_apt_cache() -> Vec<String> {
    vec![
        "apt-get clean".into(),
        "apt-get autoremove --purge -y".into(),
    ]
}

/// Clean thumbnail cache (requires sudo for system-wide, user for local)
pub fn clean_thumbnails() -> Option<String> {
    dirs::home_dir().map(|home| {
        format!("rm -rf {}/.cache/thumbnails/thumb-* 2>/dev/null || true", home.display())
    })
}

/// Clean journal logs with size limit (requires sudo)
pub fn clean_journal(max_size_mb: u32) -> String {
    format!("journalctl --vacuum-size={}M", max_size_mb)
}

/// Clean temp files safely (requires sudo)
/// Uses find with -delete to avoid "directory not empty" issues
pub fn clean_temp() -> Vec<String> {
    vec![
        // Remove files older than 1 hour in /tmp (preserve active sessions)
        "find /tmp -mindepth 1 -mmin +60 -delete 2>/dev/null || true".into(),
        // Remove files older than 1 hour in /var/tmp
        "find /var/tmp -mindepth 1 -mmin +60 -delete 2>/dev/null || true".into(),
        // Clear X11 lock files
        "rm -f /tmp/.X*-lock 2>/dev/null || true".into(),
        // Clear user crontab temp files
        "rm -f /tmp/crontab.* 2>/dev/null || true".into(),
    ]
}

/// Remove orphaned package configs (requires sudo)
pub fn clean_orphans() -> String {
    "dpkg -l | grep ^rc | awk '{print $2}' | xargs -r dpkg --purge 2>/dev/null || true".into()
}

/// Find duplicate files using fdupes
pub async fn find_duplicates(path: &PathBuf) -> Result<Vec<Vec<PathBuf>>, std::io::Error> {
    let output = Command::new("fdupes")
        .args(["-r", "-f", &path.to_string_lossy()])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut groups: Vec<Vec<PathBuf>> = Vec::new();
    let mut current_group: Vec<PathBuf> = Vec::new();

    for line in stdout.lines() {
        if line.is_empty() {
            if !current_group.is_empty() {
                groups.push(current_group);
                current_group = Vec::new();
            }
        } else {
            current_group.push(PathBuf::from(line));
        }
    }

    if !current_group.is_empty() {
        groups.push(current_group);
    }

    Ok(groups)
}