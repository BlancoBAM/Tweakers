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
    let terminals = [
        ("cosmic-term", &["-e", "rclone", "config"] as &[&str]),
        ("gnome-terminal", &["--", "rclone", "config"]),
        ("konsole", &["-e", "rclone", "config"]),
        ("xfce4-terminal", &["-e", "rclone", "config"]),
        ("terminator", &["-e", "rclone", "config"]),
        ("xterm", &["-e", "rclone", "config"]),
    ];

    for (term, args) in terminals {
        if which::which(term).is_ok() {
            Command::new(term)
                .args(args)
                .spawn()?;
            return Ok(());
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No terminal emulator found",
    ))
}

/// Sync files to remote using rclone with proper progress tracking
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
        let progress = (i as f32 + 1.0) / total;  // Correct: (completed / total)
        let filename = file.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());

        progress_callback(progress, format!("Uploading: {}", filename));

        let dest = if file.is_dir() {
            format!("{}:{}", remote, file.file_name().unwrap().to_string_lossy())
        } else {
            format!("{}:{}", remote, filename)
        };

        let status = Command::new("rclone")
            .args(["copyto", "--progress", &file.to_string_lossy(), &dest])
            .output();

        match status {
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

/// Get remote info
pub fn get_remote_info(remote: &str) -> Result<String, std::io::Error> {
    let output = Command::new("rclone")
        .args(["about", &format!("{}:", remote)])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}