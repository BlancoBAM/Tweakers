/// cosmic_apps.rs — transparent launcher integration for COSMIC desktop utilities.
///
/// Manages detection and subprocess spawning for:
///   - cosmic-tweaks  (github.com/cosmic-utils/tweaks)
///   - fan-control    (github.com/wiiznokes/fan-control)
///   - cosmic-color-picker (github.com/PixelDoted/cosmic-color-picker)
///
/// Each app is launched as a detached subprocess — visible as a focused window
/// but not permanently pinned to the taskbar.
use std::path::PathBuf;
use std::process::{Command, Stdio};

// ─────────────────────────────── Detection ────────────────────────────────

/// Availability of the three COSMIC companion apps.
#[derive(Debug, Clone, Default)]
pub struct CosmicAppsAvailability {
    pub cosmic_tweaks_path: Option<PathBuf>,
    pub fan_control_path: Option<PathBuf>,
    pub color_picker_path: Option<PathBuf>,
}

/// Candidate binary names to search for each app.
const COSMIC_TWEAKS_BINS: &[&str] = &["cosmic-tweaks", "cosmic_tweaks"];
const FAN_CONTROL_BINS: &[&str] = &["fan-control", "fan_control"];
const COLOR_PICKER_BINS: &[&str] = &[
    "cosmic-color-picker",
    "cosmic_color_picker",
    "color-picker",
];

/// Detect all three COSMIC companion apps. Checks $PATH first, then
/// common install locations (~/.cargo/bin, ~/.local/bin, /usr/local/bin).
pub fn detect_apps() -> CosmicAppsAvailability {
    CosmicAppsAvailability {
        cosmic_tweaks_path: find_binary(COSMIC_TWEAKS_BINS),
        fan_control_path: find_binary(FAN_CONTROL_BINS),
        color_picker_path: find_binary(COLOR_PICKER_BINS),
    }
}

fn find_binary(names: &[&str]) -> Option<PathBuf> {
    // 1. Try which / PATH lookup
    for name in names {
        if let Ok(p) = which::which(name) {
            return Some(p);
        }
    }
    // 2. Extended search locations
    let extra_dirs: Vec<PathBuf> = {
        let mut v = vec![
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ];
        if let Some(h) = dirs::home_dir() {
            v.push(h.join(".cargo/bin"));
            v.push(h.join(".local/bin"));
        }
        v
    };
    for dir in extra_dirs {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

// ─────────────────────────────── Launchers ────────────────────────────────

/// Launch cosmic-tweaks as a detached window.
pub fn launch_cosmic_tweaks(path: &PathBuf) -> Result<(), String> {
    spawn_detached(path, &[])
}

/// Launch fan-control as a detached window.
pub fn launch_fan_control(path: &PathBuf) -> Result<(), String> {
    spawn_detached(path, &[])
}

/// Launch cosmic-color-picker as a detached window.
pub fn launch_color_picker(path: &PathBuf) -> Result<(), String> {
    spawn_detached(path, &[])
}

/// Spawn `binary` with `args` fully detached — no terminal, no parent process
/// dependency, no inherited stdio. The child window will appear on-screen as a
/// normal windowed application.
fn spawn_detached(path: &PathBuf, args: &[&str]) -> Result<(), String> {
    Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch {}: {}", path.display(), e))
}

// ─────────────────────────────── Install hints ────────────────────────────

pub fn cosmic_tweaks_install_hint() -> &'static str {
    "cosmic-tweaks not found.\n\
     Install from: https://github.com/cosmic-utils/tweaks\n\
     Or try: cargo install cosmic-tweaks"
}

pub fn fan_control_install_hint() -> &'static str {
    "fan-control not found.\n\
     Install from: https://github.com/wiiznokes/fan-control\n\
     Or try: cargo install fan-control"
}

pub fn color_picker_install_hint() -> &'static str {
    "cosmic-color-picker not found.\n\
     Install from: https://github.com/PixelDoted/cosmic-color-picker\n\
     Or try: cargo install cosmic-color-picker"
}
