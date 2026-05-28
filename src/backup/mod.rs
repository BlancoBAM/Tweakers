#![allow(dead_code)]
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Default, Debug)]
pub struct BackupState {
    pub remote_name: Option<String>,
    pub remote_type: Option<String>,
    pub queued_files: Vec<PathBuf>,
    pub is_configured: bool,
    pub apt_cache_selected: bool,
    pub thumbnail_selected: bool,
    pub journal_selected: bool,
    pub temp_selected: bool,
    pub orphan_selected: bool,
}

/// Check if rclone is installed
pub fn check_rclone_installed() -> bool {
    which::which("rclone").is_ok()
}

/// List configured rclone remotes
pub fn list_remotes() -> Result<Vec<String>, std::io::Error> {
    let output = Command::new("rclone").args(["listremotes"]).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|s| s.trim_end_matches(':').to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Open rclone config in a terminal emulator
pub fn open_rclone_config() -> Result<(), std::io::Error> {
    let terminals: &[(&str, &[&str])] = &[
        ("cosmic-term", &["-e", "rclone", "config"]),
        ("gnome-terminal", &["--", "rclone", "config"]),
        ("konsole", &["-e", "rclone", "config"]),
        ("xfce4-terminal", &["-e", "rclone", "config"]),
        ("xterm", &["-e", "rclone", "config"]),
    ];
    for (term, args) in terminals {
        if which::which(term).is_ok() {
            Command::new(term).args(*args).spawn()?;
            return Ok(());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No terminal emulator found to open rclone config",
    ))
}

/// Open a native file picker. Tries:
///   1. xdg-desktop-portal via ashpd (Wayland native)
///   2. zenity (GTK)
///   3. kdialog (KDE)
pub async fn pick_files_native() -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    // Try ashpd portal first
    match pick_files_ashpd().await {
        Ok(paths) => return Ok(paths),
        Err(e) => {
            log::warn!("ashpd file chooser failed: {}, falling back to CLI pickers", e);
        }
    }

    // Try zenity first (most commonly available on GNOME/COSMIC)
    if which::which("zenity").is_ok() {
        return pick_files_zenity().await;
    }
    // Try kdialog (KDE environments)
    if which::which("kdialog").is_ok() {
        return pick_files_kdialog().await;
    }
    // Try yad (Yet Another Dialog)
    if which::which("yad").is_ok() {
        return pick_files_yad().await;
    }
    Err("No file picker found. Please install zenity: sudo apt install zenity".into())
}

async fn pick_files_ashpd() -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    use ashpd::desktop::file_chooser::SelectedFiles;

    let response = SelectedFiles::open_file()
        .title("Select files to backup")
        .multiple(true)
        .send()
        .await?
        .response()?;

    let paths = response
        .uris()
        .iter()
        .filter_map(|uri| uri.to_file_path().ok())
        .collect();
    Ok(paths)
}

async fn pick_files_zenity() -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let output = tokio::process::Command::new("zenity")
        .args([
            "--file-selection",
            "--multiple",
            "--separator=\n",
            "--title=Select files to backup",
        ])
        .output()
        .await?;

    if !output.status.success() {
        return Ok(vec![]); // user cancelled
    }

    let paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .collect();
    Ok(paths)
}

async fn pick_files_kdialog() -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let output = tokio::process::Command::new("kdialog")
        .args(["--getopenfilename", ".", "*", "--multiple", "--separate-output"])
        .output()
        .await?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .collect();
    Ok(paths)
}

async fn pick_files_yad() -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let output = tokio::process::Command::new("yad")
        .args(["--file-selection", "--multiple", "--title=Select files to backup"])
        .output()
        .await?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let paths = String::from_utf8_lossy(&output.stdout)
        .split('|')
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    Ok(paths)
}

/// Sync files to remote using rclone
pub async fn sync_to_remote(
    remote: &str,
    files: &[PathBuf],
    progress_callback: impl Fn(f32, String),
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let total = files.len() as f32;
    if total == 0.0 {
        return Ok(());
    }

    for (i, file) in files.iter().enumerate() {
        let progress = (i as f32 + 1.0) / total;
        let filename = file
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());

        progress_callback(progress, format!("Uploading: {}", filename));

        let dest = format!("{}:{}", remote, filename);
        let result = Command::new("rclone")
            .args(["copyto", &file.to_string_lossy(), &dest])
            .output();

        match result {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return Err(format!("Failed to sync {}: {}", filename, stderr.trim()).into());
            }
            Err(e) => {
                return Err(format!("Failed to sync {}: {}", filename, e).into());
            }
        }
    }

    progress_callback(1.0, "Complete".into());
    Ok(())
}