/// czkawka.rs — Integration with czkawka CLI and krokiet GUI
///
/// Strategy:
///   1. Prefer launching `krokiet` (the Slint-based GUI) transparently for full-featured scans.
///   2. Fall back to `czkawka_cli` for headless/inline results.
///   3. If neither is installed, surface an install prompt.

use std::process::{Command, Stdio};

/// Describes which czkawka tools are available on the system.
#[derive(Clone, Debug, Default)]
pub struct CzkawkaAvailability {
    pub krokiet_path: Option<String>,
    pub cli_path: Option<String>,
}

impl CzkawkaAvailability {
    pub fn is_any_available(&self) -> bool {
        self.krokiet_path.is_some() || self.cli_path.is_some()
    }

    pub fn preferred_tool(&self) -> Option<&str> {
        self.krokiet_path
            .as_deref()
            .or(self.cli_path.as_deref())
    }
}

/// Scan the PATH (and common cargo bin locations) for krokiet and czkawka_cli.
pub fn detect_tools() -> CzkawkaAvailability {
    CzkawkaAvailability {
        krokiet_path: find_binary("krokiet"),
        cli_path: find_binary("czkawka_cli").or_else(|| find_binary("czkawka")),
    }
}

fn find_binary(name: &str) -> Option<String> {
    // Try `which` first
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // Also check common cargo install locations
    let home = dirs::home_dir()?;
    let cargo_bin = home.join(".cargo/bin").join(name);
    if cargo_bin.exists() {
        return Some(cargo_bin.to_string_lossy().into_owned());
    }

    None
}

/// Scan mode that can be passed to krokiet or czkawka_cli.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanMode {
    DuplicateFiles,
    EmptyFiles,
    EmptyDirectories,
    SimilarImages,
    SimilarVideos,
    SameMusic,
    InvalidSymlinks,
    BrokenFiles,
    BadExtensions,
    BigFiles,
    TemporaryFiles,
}

impl ScanMode {
    /// Human-readable name shown in the UI.
    pub fn label(&self) -> &str {
        match self {
            ScanMode::DuplicateFiles => "Duplicate Files",
            ScanMode::EmptyFiles => "Empty Files",
            ScanMode::EmptyDirectories => "Empty Directories",
            ScanMode::SimilarImages => "Similar Images",
            ScanMode::SimilarVideos => "Similar Videos",
            ScanMode::SameMusic => "Same Music",
            ScanMode::InvalidSymlinks => "Invalid Symlinks",
            ScanMode::BrokenFiles => "Broken Files",
            ScanMode::BadExtensions => "Bad Extensions",
            ScanMode::BigFiles => "Big Files",
            ScanMode::TemporaryFiles => "Temporary Files",
        }
    }

    /// Icon/emoji for this mode.
    #[allow(dead_code)]
    pub fn icon(&self) -> &str {
        match self {
            ScanMode::DuplicateFiles => "📄",
            ScanMode::EmptyFiles => "📭",
            ScanMode::EmptyDirectories => "📂",
            ScanMode::SimilarImages => "🖼",
            ScanMode::SimilarVideos => "🎬",
            ScanMode::SameMusic => "🎵",
            ScanMode::InvalidSymlinks => "🔗",
            ScanMode::BrokenFiles => "💔",
            ScanMode::BadExtensions => "🏷",
            ScanMode::BigFiles => "📦",
            ScanMode::TemporaryFiles => "🗑",
        }
    }

    /// The krokiet --tool flag value (when krokiet supports it via CLI args).
    /// Krokiet is a GUI app — we open it and the user sees a windowed interface.
    /// This is used for czkawka_cli only.
    pub fn cli_subcommand(&self) -> &str {
        match self {
            ScanMode::DuplicateFiles => "dup",
            ScanMode::EmptyFiles => "empty-files",
            ScanMode::EmptyDirectories => "empty-folders",
            ScanMode::SimilarImages => "image",
            ScanMode::SimilarVideos => "video",
            ScanMode::SameMusic => "music",
            ScanMode::InvalidSymlinks => "symlinks",
            ScanMode::BrokenFiles => "broken",
            ScanMode::BadExtensions => "ext",
            ScanMode::BigFiles => "big",
            ScanMode::TemporaryFiles => "temp",
        }
    }
}

/// Launch krokiet GUI transparently (no taskbar entry needed — it will appear as a window).
/// `scan_mode` is for documentation/logging only; krokiet doesn't support CLI tab switching yet.
pub fn launch_krokiet(krokiet_path: &str, scan_mode: &ScanMode) -> Result<(), String> {
    log::info!(
        "Launching krokiet for scan mode: {:?} at {}",
        scan_mode,
        krokiet_path
    );

    Command::new(krokiet_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Prevent it from appearing in the application dock/taskbar on some DEs
        // by not inheriting the parent session
        .spawn()
        .map_err(|e| format!("Failed to launch krokiet: {}", e))?;

    Ok(())
}

/// Run czkawka_cli for a scan and capture structured output.
/// Returns a list of result lines (file paths, grouped by blank lines).
pub async fn run_cli_scan(
    cli_path: &str,
    mode: &ScanMode,
    directories: &[std::path::PathBuf],
) -> Result<Vec<ResultGroup>, String> {
    let dir_args: Vec<String> = directories
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    let mut cmd = tokio::process::Command::new(cli_path);
    cmd.arg(mode.cli_subcommand());

    for dir in &dir_args {
        cmd.arg("-d").arg(dir);
    }

    // Request JSON output if the tool supports it (czkawka_cli >= 6.x uses --json flag for some modes)
    // For safety, use plain text output
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run czkawka_cli: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(parse_cli_output(&stdout))
}

/// A group of files identified together (e.g., a set of duplicates).
#[derive(Clone, Debug)]
pub struct ResultGroup {
    pub files: Vec<String>,
}

fn parse_cli_output(output: &str) -> Vec<ResultGroup> {
    let mut groups = Vec::new();
    let mut current: Vec<String> = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                groups.push(ResultGroup {
                    files: current.clone(),
                });
                current.clear();
            }
        } else if trimmed.starts_with('/') || trimmed.contains('\\') {
            // Looks like a file path
            current.push(trimmed.to_string());
        }
    }

    if !current.is_empty() {
        groups.push(ResultGroup { files: current });
    }

    groups
}

/// Returns a human-friendly install hint.
pub fn install_hint() -> String {
    format!(
        "Neither krokiet nor czkawka_cli was found.\n\
        Install via:\n  cargo install krokiet --locked\n\
        or download from: https://github.com/qarmin/czkawka/releases"
    )
}

/// All available scan modes as a list, matching the krokiet sidebar order.
#[allow(dead_code)]
pub fn all_scan_modes() -> Vec<ScanMode> {
    vec![
        ScanMode::DuplicateFiles,
        ScanMode::EmptyFiles,
        ScanMode::EmptyDirectories,
        ScanMode::SimilarImages,
        ScanMode::SimilarVideos,
        ScanMode::SameMusic,
        ScanMode::InvalidSymlinks,
        ScanMode::BrokenFiles,
        ScanMode::BadExtensions,
        ScanMode::BigFiles,
        ScanMode::TemporaryFiles,
    ]
}
