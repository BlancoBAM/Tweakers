use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Default, Debug)]
pub struct BackupState {
    pub remote_name: Option<String>,
    pub remote_type: Option<String>,
    pub queued_files: Vec<PathBuf>,
    pub is_configured: bool,
}

/// Check if rclone is installed
pub fn check_rclone_installed() -> bool {
    which::which("rclone").is_ok()
}

/// List configured rclone remotes
pub fn list_remotes() -> Result<Vec<String>, std::io::Error> {
    let output = Command::new("rclone")
        .args(["listremotes"])
        .output()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines()
        .map(|s| s.trim_end_matches(':').to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Open rclone config in terminal
pub fn open_rclone_config() -> Result<(), std::io::Error> {
    // Try to open in the user's preferred terminal
    let terminals = [
        ("cosmic-term", &["-e", "rclone", "config"]),
        ("gnome-terminal", &["--", "rclone", "config"]),
        ("konsole", &["-e", "rclone", "config"]),
        ("xterm", &["-e", "rclone", "config"]),
    ];
    
    for (term, args) in terminals {
        if which::which(term).is_ok() {
            Command::new(term)
                .args(args.iter())
                .spawn()?;
            return Ok(());
        }
    }
    
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No terminal emulator found",
    ))
}

/// Sync files to remote using rclone
pub async fn sync_to_remote(
    remote: &str,
    files: &[PathBuf],
    progress_callback: impl Fn(f32, String),
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let total = files.len();
    
    for (i, file) in files.iter().enumerate() {
        let progress = (i as f32 + 0.5) / total as f32;
        let filename = file.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        
        progress_callback(progress, format!("Uploading: {}", filename));
        
        let dest = if file.is_dir() {
            format!("{}:{}", remote, file.file_name().unwrap().to_string_lossy())
        } else {
            format!("{}:", remote)
        };
        
        let status = Command::new("rclone")
            .args(["copy", "--progress", &file.to_string_lossy(), &dest])
            .status()?;
        
        if !status.success() {
            return Err(format!("Failed to sync: {}", filename).into());
        }
    }
    
    progress_callback(1.0, "Complete".into());
    Ok(())
}

/// Get remote info
pub fn get_remote_info(remote: &str) -> Result<String, std::io::Error> {
    let output = Command::new("rclone")
        .args(["about", &format!("{}:", remote)])
        .output()?;
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
