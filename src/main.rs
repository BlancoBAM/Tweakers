mod tweaks;
mod cleaners;
mod backup;
mod benchmark;
mod sudo_handler;
mod czkawka;
mod cosmic_apps;
mod cosmic_themes;
mod resource_monitor;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

slint::include_modules!();

#[derive(Clone, Default)]
pub struct AppState {
    pub tweaks: tweaks::TweakSettings,
    pub cleaner: cleaners::CleanerState,
    pub backup: backup::BackupState,
    pub benchmark: benchmark::BenchmarkResults,
    pub pending_action: PendingAction,
    pub askpass_path: String,
    pub czkawka: czkawka::CzkawkaAvailability,
    pub cosmic_apps: cosmic_apps::CosmicAppsAvailability,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub enum PendingAction {
    #[default]
    None,
    ApplyTweaks,
    ResetDefaults,
    CleanSystem,
}

// ─────────────────────────────── Toast helper ─────────────────────────────

/// Show a slide-in toast notification for 3 seconds then hide it.
fn show_toast(app_weak: &slint::Weak<MainWindow>, message: &str, success: bool) {
    let msg = message.to_string();
    let aw = app_weak.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(app) = aw.upgrade() {
            app.set_toast_message(msg.into());
            app.set_toast_success(success);
            app.set_show_toast(true);
        }
    })
    .ok();

    let aw2 = app_weak.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        slint::invoke_from_event_loop(move || {
            if let Some(app) = aw2.upgrade() {
                app.set_show_toast(false);
            }
        })
        .ok();
    });
}

// ─────────────────────────────── Main ─────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let app = MainWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::default()));

    {
        let mut s = state.lock().await;
        s.tweaks = tweaks::load_current_settings().await;
        s.askpass_path = String::new();

        // Detect czkawka / krokiet availability
        s.czkawka = czkawka::detect_tools();
        log::info!(
            "czkawka detection: krokiet={:?}, cli={:?}",
            s.czkawka.krokiet_path,
            s.czkawka.cli_path
        );

        // Detect COSMIC companion apps
        s.cosmic_apps = cosmic_apps::detect_apps();
        log::info!(
            "cosmic apps: tweaks={:?}, fan={:?}, color={:?}",
            s.cosmic_apps.cosmic_tweaks_path,
            s.cosmic_apps.fan_control_path,
            s.cosmic_apps.color_picker_path,
        );

        if backup::check_rclone_installed() {
            if let Ok(remotes) = backup::list_remotes() {
                if let Some(first) = remotes.first() {
                    s.backup.remote_name = Some(first.clone());
                    s.backup.is_configured = true;
                }
            }
        }
    }

    initialize_ui(&app, &state).await;

    app.set_sysbench_installed(benchmark::check_sysbench_installed());
    app.set_fio_installed(benchmark::check_fio_installed());

    setup_tweak_callbacks(&app, state.clone());
    setup_cleaner_callbacks(&app, state.clone());
    setup_backup_callbacks(&app, state.clone());
    setup_benchmark_callbacks(&app, state.clone());
    setup_sudo_callbacks(&app, state.clone());
    setup_cosmic_app_callbacks(&app, state.clone());
    setup_cosmic_theme_callbacks(&app);

    // Start live resource monitor — updates CPU/RAM every 2s
    resource_monitor::start_monitor_task(app.as_weak());

    app.run()?;

    Ok(())
}

// ─────────────────────────────── UI init ──────────────────────────────────

async fn initialize_ui(app: &MainWindow, state: &Arc<Mutex<AppState>>) {
    let s = state.lock().await;
    let settings = &s.tweaks;

    let gov_index = match settings.cpu_governor.as_str() {
        "powersave" => 0,
        "schedutil" => 1,
        "performance" => 2,
        _ => 1,
    };
    app.set_cpu_governor_index(gov_index);
    app.set_turbo_enabled(settings.turbo_enabled);
    app.set_swappiness(settings.swappiness as f32);
    app.set_vfs_cache_pressure(settings.vfs_cache_pressure as f32);
    app.set_zram_enabled(settings.zram_enabled);

    let io_index = match settings.io_scheduler.as_str() {
        "none" => 0,
        "mq-deadline" => 1,
        "kyber" => 2,
        _ => 1,
    };
    app.set_io_scheduler_index(io_index);
    app.set_trim_enabled(settings.trim_enabled);

    app.set_intel_guc_enabled(settings.intel_guc);
    app.set_intel_psr_enabled(settings.intel_psr);
    app.set_intel_fbc_enabled(settings.intel_fbc);

    app.set_wifi_powersave(settings.wifi_powersave);
    app.set_audio_powersave(settings.audio_powersave);

    app.set_tcp_fastopen(settings.tcp_fastopen as f32);
    app.set_tcp_low_latency(settings.tcp_low_latency);
    app.set_bbr_enabled(settings.bbr_enabled);

    app.set_watchdog_enabled(settings.watchdog_enabled);
    let thp_index = match settings.transparent_hugepages.as_str() {
        "always" => 0,
        "madvise" => 1,
        "never" => 2,
        _ => 1,
    };
    app.set_thp_index(thp_index);

    if let Some(remote) = &s.backup.remote_name {
        app.set_remote_name(remote.clone().into());
        app.set_is_configured(true);
    }

    // czkawka / krokiet availability flags
    let krokiet_avail = s.czkawka.krokiet_path.is_some();
    let cli_avail = s.czkawka.cli_path.is_some();
    app.set_krokiet_available(krokiet_avail);
    app.set_czkawka_cli_available(cli_avail);
    app.set_czkawka_status(
        if krokiet_avail {
            "krokiet found ✓".into()
        } else if cli_avail {
            "czkawka_cli found ✓ (install krokiet for GUI)".into()
        } else {
            "Not installed — install krokiet for full functionality".into()
        },
    );

    // COSMIC companion app availability flags
    app.set_cosmic_tweaks_available(s.cosmic_apps.cosmic_tweaks_path.is_some());
    app.set_fan_control_available(s.cosmic_apps.fan_control_path.is_some());
    app.set_color_picker_available(s.cosmic_apps.color_picker_path.is_some());
}

// ─────────────────────────────── Tweak callbacks ──────────────────────────

fn setup_tweak_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_apply_tweaks(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();

        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                let mut new_settings = {
                    let s = futures::executor::block_on(state.lock());
                    s.tweaks.clone()
                };

                new_settings.cpu_governor = match app.get_cpu_governor_index() {
                    0 => "powersave".into(),
                    1 => "schedutil".into(),
                    2 => "performance".into(),
                    _ => "schedutil".into(),
                };
                new_settings.turbo_enabled = app.get_turbo_enabled();
                new_settings.swappiness = app.get_swappiness() as u32;
                new_settings.vfs_cache_pressure = app.get_vfs_cache_pressure() as u32;
                new_settings.zram_enabled = app.get_zram_enabled();
                new_settings.io_scheduler = match app.get_io_scheduler_index() {
                    0 => "none".into(),
                    1 => "mq-deadline".into(),
                    2 => "kyber".into(),
                    _ => "mq-deadline".into(),
                };
                new_settings.trim_enabled = app.get_trim_enabled();
                new_settings.intel_guc = app.get_intel_guc_enabled();
                new_settings.intel_psr = app.get_intel_psr_enabled();
                new_settings.intel_fbc = app.get_intel_fbc_enabled();
                new_settings.wifi_powersave = app.get_wifi_powersave();
                new_settings.audio_powersave = app.get_audio_powersave();
                new_settings.tcp_fastopen = app.get_tcp_fastopen() as u32;
                new_settings.tcp_low_latency = app.get_tcp_low_latency();
                new_settings.bbr_enabled = app.get_bbr_enabled();
                new_settings.watchdog_enabled = app.get_watchdog_enabled();
                new_settings.transparent_hugepages = match app.get_thp_index() {
                    0 => "always".into(),
                    1 => "madvise".into(),
                    2 => "never".into(),
                    _ => "madvise".into(),
                };

                tokio::spawn(async move {
                    let mut s = state.lock().await;
                    s.tweaks = new_settings;
                    s.pending_action = PendingAction::ApplyTweaks;
                });

                app.set_sudo_action("Apply system tweaks and optimizations".into());
                app.set_show_sudo_dialog(true);
            }
        })
        .ok();
    });

    let app_weak = app.as_weak();
    app.on_reset_defaults(move || {
        let app_weak = app_weak.clone();
        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_sudo_action("Reset all settings to system defaults".into());
                app.set_show_sudo_dialog(true);
            }
        })
        .ok();
    });

    let app_weak = app.as_weak();
    app.on_set_profile(move |profile| {
        if let Some(app) = app_weak.upgrade() {
            match profile.as_str() {
                "battery" => {
                    app.set_cpu_governor_index(0);
                    app.set_turbo_enabled(false);
                    app.set_wifi_powersave(true);
                    app.set_audio_powersave(true);
                    app.set_intel_psr_enabled(true);
                    app.set_intel_fbc_enabled(true);
                }
                "balanced" => {
                    app.set_cpu_governor_index(1);
                    app.set_turbo_enabled(true);
                    app.set_wifi_powersave(false);
                    app.set_audio_powersave(true);
                    app.set_intel_psr_enabled(true);
                    app.set_intel_fbc_enabled(true);
                }
                "performance" => {
                    app.set_cpu_governor_index(2);
                    app.set_turbo_enabled(true);
                    app.set_wifi_powersave(false);
                    app.set_audio_powersave(false);
                    app.set_intel_psr_enabled(false);
                    app.set_intel_fbc_enabled(false);
                    app.set_intel_guc_enabled(true);
                }
                "presentation" => {
                    app.set_cpu_governor_index(1);
                    app.set_turbo_enabled(true);
                    app.set_intel_psr_enabled(false);
                }
                _ => {}
            }
        }
    });
}

// ─────────────────────────────── Cleaner callbacks ────────────────────────

fn setup_cleaner_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_scan_cleaner(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();

        if let Some(app) = app_weak.upgrade() {
            app.set_is_scanning(true);
        }

        tokio::spawn(async move {
            let sizes = cleaners::scan_all().await;

            {
                let mut s = state.lock().await;
                s.cleaner = sizes;
            }

            slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_apt_cache_size(sizes.apt_cache_mb as f32);
                    app.set_thumbnail_cache_size(sizes.thumbnail_cache_mb as f32);
                    app.set_journal_size(sizes.journal_mb as f32);
                    app.set_temp_size(sizes.temp_mb as f32);
                    app.set_orphan_packages(sizes.orphan_count as i32);
                    app.set_is_scanning(false);
                }
            })
            .ok();
        });
    });

    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_clean_selected(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();

        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                let apt = app.get_apt_cache_selected();
                let thumb = app.get_thumbnail_selected();
                let journal = app.get_journal_selected();
                let temp = app.get_temp_selected();
                let orphans = app.get_orphan_selected();

                tokio::spawn(async move {
                    let mut s = state.lock().await;
                    s.cleaner.apt_cache_selected = apt;
                    s.cleaner.thumbnail_selected = thumb;
                    s.cleaner.journal_selected = journal;
                    s.cleaner.temp_selected = temp;
                    s.cleaner.orphan_selected = orphans;
                    s.pending_action = PendingAction::CleanSystem;
                });

                app.set_sudo_action("Clean selected system files and caches".into());
                app.set_show_sudo_dialog(true);
            }
        })
        .ok();
    });

    let app_weak2 = app.as_weak();
    let state_cz = state.clone();

    app.on_find_duplicates(move || {
        let app_weak3 = app_weak2.clone();
        let state = state_cz.clone();

        tokio::spawn(async move {
            let availability = {
                let s = state.lock().await;
                s.czkawka.clone()
            };

            if let Some(krokiet) = &availability.krokiet_path {
                match czkawka::launch_krokiet(krokiet, &czkawka::ScanMode::DuplicateFiles) {
                    Ok(()) => log::info!("krokiet launched for duplicate scan"),
                    Err(e) => {
                        log::error!("Failed to launch krokiet: {}", e);
                        run_cli_duplicate_scan(&app_weak3, &availability).await;
                    }
                }
            } else if availability.cli_path.is_some() {
                run_cli_duplicate_scan(&app_weak3, &availability).await;
            } else {
                let hint = czkawka::install_hint();
                log::warn!("{}", hint);
                slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak3.upgrade() {
                        app.set_czkawka_status(hint.replace('\n', " | ").into());
                    }
                })
                .ok();
            }
        });
    });

    let app_weak_scan = app.as_weak();
    let state_scan = state.clone();

    app.on_launch_czkawka_scan(move |mode_str| {
        let app_weak = app_weak_scan.clone();
        let state = state_scan.clone();
        let mode_str = mode_str.to_string();

        tokio::spawn(async move {
            let availability = {
                let s = state.lock().await;
                s.czkawka.clone()
            };

            let mode = parse_scan_mode(&mode_str);

            if let Some(krokiet) = &availability.krokiet_path {
                match czkawka::launch_krokiet(krokiet, &mode) {
                    Ok(()) => {
                        log::info!("krokiet launched for mode: {}", mode_str);
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_czkawka_status("krokiet opened ✓".into());
                            }
                        })
                        .ok();
                    }
                    Err(e) => {
                        let msg = format!("Launch failed: {}", e);
                        log::error!("{}", msg);
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_czkawka_status(msg.into());
                            }
                        })
                        .ok();
                    }
                }
            } else if let Some(cli) = &availability.cli_path {
                slint::invoke_from_event_loop({
                    let app_weak = app_weak.clone();
                    let mode_label = mode.label().to_string();
                    move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_czkawka_status(format!("Scanning: {}…", mode_label).into());
                            app.set_is_czkawka_scanning(true);
                        }
                    }
                })
                .ok();

                let home = dirs::home_dir().unwrap_or_default();
                match czkawka::run_cli_scan(cli, &mode, &[home]).await {
                    Ok(groups) => {
                        let count = groups.len();
                        let total_files: usize = groups.iter().map(|g| g.files.len()).sum();
                        let status = format!(
                            "Found {} groups ({} files) | {}",
                            count,
                            total_files,
                            mode.label()
                        );
                        log::info!("{}", status);
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_czkawka_status(status.into());
                                app.set_czkawka_result_count(count as i32);
                                app.set_is_czkawka_scanning(false);
                            }
                        })
                        .ok();
                    }
                    Err(e) => {
                        let msg = format!("Scan error: {}", e);
                        log::error!("{}", msg);
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_czkawka_status(msg.into());
                                app.set_is_czkawka_scanning(false);
                            }
                        })
                        .ok();
                    }
                }
            } else {
                let hint = czkawka::install_hint();
                slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_czkawka_status(hint.replace('\n', " | ").into());
                    }
                })
                .ok();
            }
        });
    });

    let app_weak_install = app.as_weak();
    app.on_install_krokiet(move || {
        let app_weak = app_weak_install.clone();
        tokio::spawn(async move {
            slint::invoke_from_event_loop({
                let app_weak = app_weak.clone();
                move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_czkawka_status(
                            "Installing krokiet via cargo… this may take a while".into(),
                        );
                        app.set_is_czkawka_scanning(true);
                    }
                }
            })
            .ok();

            let result = tokio::process::Command::new("cargo")
                .args(["install", "krokiet", "--locked"])
                .output()
                .await;

            let (status_msg, success) = match result {
                Ok(out) if out.status.success() => (
                    "krokiet installed successfully! ✓".to_string(),
                    true,
                ),
                Ok(out) => (
                    format!(
                        "Install failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                            .lines()
                            .last()
                            .unwrap_or("unknown error")
                    ),
                    false,
                ),
                Err(e) => (format!("cargo not found: {}", e), false),
            };

            log::info!("{}", status_msg);
            slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_czkawka_status(status_msg.into());
                    app.set_is_czkawka_scanning(false);
                    if success {
                        let avail = czkawka::detect_tools();
                        app.set_krokiet_available(avail.krokiet_path.is_some());
                        app.set_czkawka_cli_available(avail.cli_path.is_some());
                    }
                }
            })
            .ok();
        });
    });

    // Open a URL in the system default browser
    app.on_open_site(move |url| {
        let url = url.to_string();
        std::thread::spawn(move || {
            let _ = std::process::Command::new("xdg-open")
                .arg(&url)
                .spawn();
        });
    });
}

fn parse_scan_mode(s: &str) -> czkawka::ScanMode {
    match s {
        "dup" | "duplicates" => czkawka::ScanMode::DuplicateFiles,
        "empty-files" => czkawka::ScanMode::EmptyFiles,
        "empty-dirs" => czkawka::ScanMode::EmptyDirectories,
        "similar-images" | "image" => czkawka::ScanMode::SimilarImages,
        "similar-videos" | "video" => czkawka::ScanMode::SimilarVideos,
        "same-music" | "music" => czkawka::ScanMode::SameMusic,
        "symlinks" => czkawka::ScanMode::InvalidSymlinks,
        "broken" => czkawka::ScanMode::BrokenFiles,
        "bad-ext" | "ext" => czkawka::ScanMode::BadExtensions,
        "big" => czkawka::ScanMode::BigFiles,
        "temp" => czkawka::ScanMode::TemporaryFiles,
        _ => czkawka::ScanMode::DuplicateFiles,
    }
}

async fn run_cli_duplicate_scan(
    app_weak: &slint::Weak<MainWindow>,
    availability: &czkawka::CzkawkaAvailability,
) {
    if let Some(cli) = &availability.cli_path {
        let home = dirs::home_dir().unwrap_or_default();
        match czkawka::run_cli_scan(cli, &czkawka::ScanMode::DuplicateFiles, &[home]).await {
            Ok(groups) => {
                let count = groups.len();
                let total: usize = groups.iter().map(|g| g.files.len()).sum();
                let msg = format!("Found {} duplicate groups, {} files", count, total);
                log::info!("{}", msg);
                let app_weak = app_weak.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_czkawka_result_count(count as i32);
                        app.set_czkawka_status(msg.into());
                    }
                })
                .ok();
            }
            Err(e) => log::error!("CLI scan error: {}", e),
        }
    }
}

// ─────────────────────────────── Backup callbacks ─────────────────────────

fn setup_backup_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    app.on_configure_rclone(move || {
        if let Err(e) = backup::open_rclone_config() {
            log::error!("Failed to open rclone config: {}", e);
        }
    });

    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_select_backup_files(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();

        tokio::spawn(async move {
            match backup::pick_files_native().await {
                Ok(files) if !files.is_empty() => {
                    let file_count = files.len();

                    // Collect display names for the UI
                    let display_names: Vec<slint::SharedString> = files
                        .iter()
                        .map(|p| {
                            p.to_string_lossy().to_string().into()
                        })
                        .collect();

                    {
                        let mut s = state.lock().await;
                        s.backup.queued_files = files;
                    }

                    log::info!("Selected {} files for backup", file_count);

                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            let model: slint::ModelRc<slint::SharedString> =
                                slint::ModelRc::new(slint::VecModel::from(display_names));
                            app.set_queued_file_names(model);
                        }
                    })
                    .ok();
                }
                Ok(_) => {
                    // User cancelled — do nothing
                    log::info!("File picker cancelled");
                }
                Err(e) => {
                    log::error!("File picker error: {}", e);
                    let msg = format!("File picker error: {}", e);
                    let app_weak2 = app_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak2.upgrade() {
                            app.set_toast_message(msg.into());
                            app.set_toast_success(false);
                            app.set_show_toast(true);
                        }
                    })
                    .ok();
                }
            }
        });
    });

    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_start_backup(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();

        tokio::spawn(async move {
            let (remote, files) = {
                let s = state.lock().await;
                (s.backup.remote_name.clone(), s.backup.queued_files.clone())
            };

            if let Some(remote) = remote {
                if !files.is_empty() {
                    slint::invoke_from_event_loop({
                        let app_weak = app_weak.clone();
                        move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_is_transferring(true);
                                app.set_transfer_progress(0.0);
                            }
                        }
                    })
                    .ok();

                    let app_weak_progress = app_weak.clone();
                    match backup::sync_to_remote(&remote, &files, |progress, _status| {
                        let app_weak = app_weak_progress.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_transfer_progress(progress);
                            }
                        })
                        .ok();
                    })
                    .await
                    {
                        Ok(()) => {
                            show_toast(&app_weak, "Backup completed successfully! ✓", true);
                            slint::invoke_from_event_loop(move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_is_transferring(false);
                                    app.set_transfer_progress(1.0);
                                }
                            })
                            .ok();
                        }
                        Err(e) => {
                            log::error!("Backup failed: {}", e);
                            let msg = format!("Backup failed: {}", e);
                            show_toast(&app_weak, &msg, false);
                            slint::invoke_from_event_loop(move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_is_transferring(false);
                                }
                            })
                            .ok();
                        }
                    }
                }
            }
        });
    });
}

// ─────────────────────────────── Benchmark callbacks ──────────────────────

fn setup_benchmark_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_run_benchmark(move |bench_type| {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();
        let bench_type = bench_type.to_string();

        slint::invoke_from_event_loop({
            let app_weak = app_weak.clone();
            let bench_type = bench_type.clone();
            move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_is_running(true);
                    app.set_current_test(bench_type.into());
                }
            }
        })
        .ok();

        tokio::spawn(async move {
            let default = benchmark::BenchmarkResults::default();

            let results = match bench_type.as_str() {
                "all" => benchmark::run_all().await,
                "cpu-single" => {
                    let score = benchmark::run_cpu_single().await;
                    let mut s = state.lock().await;
                    s.benchmark.cpu_single = score;
                    s.benchmark.with_cpu_single(score)
                }
                "cpu-multi" => {
                    let score = benchmark::run_cpu_multi().await;
                    let mut s = state.lock().await;
                    s.benchmark.cpu_multi = score;
                    s.benchmark.with_cpu_multi(score)
                }
                "memory" => {
                    let score = benchmark::run_memory().await;
                    let mut s = state.lock().await;
                    s.benchmark.memory = score;
                    s.benchmark.with_memory(score)
                }
                "disk" => {
                    let (seq, rand) = benchmark::run_disk().await;
                    let mut s = state.lock().await;
                    s.benchmark.disk_sequential = seq;
                    s.benchmark.disk_random = rand;
                    s.benchmark.with_disk(seq, rand)
                }
                _ => default,
            };

            if bench_type == "all" {
                let mut s = state.lock().await;
                s.benchmark = results;
            }

            slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_cpu_single_score(results.cpu_single as f32);
                    app.set_cpu_multi_score(results.cpu_multi as f32);
                    app.set_memory_score(results.memory as f32);
                    app.set_disk_seq_score(results.disk_sequential as f32);
                    app.set_disk_rand_score(results.disk_random as f32);
                    app.set_is_running(false);
                    app.set_current_test(slint::SharedString::new());
                }
            })
            .ok();
        });
    });
}

// ─────────────────────────────── Sudo callbacks ───────────────────────────

fn setup_sudo_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_sudo_authenticate(move |password| {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();
        let password = password.to_string();

        slint::invoke_from_event_loop({
            let app_weak = app_weak.clone();
            move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_sudo_loading(true);
                    app.set_sudo_error("".into());
                }
            }
        })
        .ok();

        tokio::spawn(async move {
            let askpass_content = format!("#!/bin/sh\necho '{}'\n", password);
            let askpass_path = format!("/tmp/tweakers-askpass-{}", std::process::id());

            if let Err(e) = tokio::fs::write(&askpass_path, &askpass_content).await {
                slint::invoke_from_event_loop({
                    let app_weak = app_weak.clone();
                    move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_sudo_loading(false);
                            app.set_sudo_error(
                                format!("Failed to create askpass helper: {}", e).into(),
                            );
                        }
                    }
                })
                .ok();
                return;
            }

            let _ = tokio::process::Command::new("chmod")
                .args(["700", &askpass_path])
                .output()
                .await;

            let verify_output = tokio::process::Command::new("bash")
                .args([
                    "-c",
                    &format!(
                        "SUDO_ASKPASS={} sudo -A -n true 2>&1",
                        askpass_path
                    ),
                ])
                .env("SUDO_ASKPASS", &askpass_path)
                .env("DISPLAY", ":0")
                .output()
                .await;

            if !verify_output
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                let _ = tokio::fs::remove_file(&askpass_path).await;
                slint::invoke_from_event_loop({
                    let app_weak = app_weak.clone();
                    move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_sudo_loading(false);
                            app.set_sudo_error(
                                "Incorrect password or no sudo privileges".into(),
                            );
                        }
                    }
                })
                .ok();
                return;
            }

            {
                let mut s = state.lock().await;
                s.askpass_path = askpass_path.clone();
            }

            let (pending, askpass) = {
                let s = state.lock().await;
                (s.pending_action.clone(), s.askpass_path.clone())
            };

            let result = match pending {
                PendingAction::ApplyTweaks => {
                    sudo_handler::authenticate_and_apply(&askpass, &password, &state).await
                }
                PendingAction::ResetDefaults => {
                    sudo_handler::reset_to_defaults(&askpass).await
                }
                PendingAction::CleanSystem => {
                    sudo_handler::clean_system(&askpass, &state).await
                }
                PendingAction::None => Ok(()),
            };

            let _ = tokio::fs::remove_file(&askpass_path).await;

            match result {
                Ok(()) => {
                    let success_msg = match pending {
                        PendingAction::ApplyTweaks => "Settings applied successfully! ✓",
                        PendingAction::ResetDefaults => "Settings reset to defaults ✓",
                        PendingAction::CleanSystem => "System cleaned successfully! ✓",
                        PendingAction::None => "Done ✓",
                    };

                    {
                        let mut s = state.lock().await;
                        s.pending_action = PendingAction::None;
                    }

                    show_toast(&app_weak, success_msg, true);

                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_show_sudo_dialog(false);
                            app.set_sudo_password("".into());
                            app.set_sudo_error("".into());
                            app.set_sudo_loading(false);

                            if pending == PendingAction::CleanSystem {
                                let app_weak2 = app_weak.clone();
                                tokio::spawn(async move {
                                    let sizes = cleaners::scan_all().await;
                                    slint::invoke_from_event_loop(move || {
                                        if let Some(app) = app_weak2.upgrade() {
                                            app.set_apt_cache_size(sizes.apt_cache_mb as f32);
                                            app.set_thumbnail_cache_size(
                                                sizes.thumbnail_cache_mb as f32,
                                            );
                                            app.set_journal_size(sizes.journal_mb as f32);
                                            app.set_temp_size(sizes.temp_mb as f32);
                                            app.set_orphan_packages(sizes.orphan_count as i32);
                                        }
                                    })
                                    .ok();
                                });
                            }
                        }
                    })
                    .ok();
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    log::error!("Operation failed: {}", error_msg);
                    show_toast(&app_weak, &format!("Operation failed: {}", error_msg), false);
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_sudo_error(error_msg.into());
                            app.set_sudo_loading(false);
                        }
                    })
                    .ok();
                }
            }
        });
    });

    let app_weak = app.as_weak();
    let state_clone = state.clone();

    app.on_sudo_cancel(move || {
        let state = state_clone.clone();

        tokio::spawn(async move {
            let mut s = state.lock().await;
            s.pending_action = PendingAction::None;
        });

        if let Some(app) = app_weak.upgrade() {
            app.set_show_sudo_dialog(false);
            app.set_sudo_password("".into());
            app.set_sudo_error("".into());
        }
    });
}

// ─────────────────────────────── COSMIC app callbacks ────────────────────

fn setup_cosmic_app_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    // cosmic-tweaks
    let state_ct = state.clone();
    let app_weak_ct = app.as_weak();
    app.on_launch_cosmic_tweaks(move || {
        let state = state_ct.clone();
        let app_weak = app_weak_ct.clone();
        tokio::spawn(async move {
            let path = {
                let s = state.lock().await;
                s.cosmic_apps.cosmic_tweaks_path.clone()
            };
            match path {
                Some(p) => match cosmic_apps::launch_cosmic_tweaks(&p) {
                    Ok(()) => log::info!("cosmic-tweaks launched"),
                    Err(e) => {
                        log::error!("{}", e);
                        let msg = e;
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_cosmic_tweaks_status(msg.into());
                            }
                        })
                        .ok();
                    }
                },
                None => {
                    let hint = cosmic_apps::cosmic_tweaks_install_hint();
                    log::warn!("{}", hint);
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_cosmic_tweaks_status(
                                "Not installed — see github.com/cosmic-utils/tweaks".into(),
                            );
                        }
                    })
                    .ok();
                }
            }
        });
    });

    // fan-control
    let state_fc = state.clone();
    let app_weak_fc = app.as_weak();
    app.on_launch_fan_control(move || {
        let state = state_fc.clone();
        let app_weak = app_weak_fc.clone();
        tokio::spawn(async move {
            let path = {
                let s = state.lock().await;
                s.cosmic_apps.fan_control_path.clone()
            };
            match path {
                Some(p) => match cosmic_apps::launch_fan_control(&p) {
                    Ok(()) => log::info!("fan-control launched"),
                    Err(e) => {
                        log::error!("{}", e);
                        let msg = e;
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_fan_control_status(msg.into());
                            }
                        })
                        .ok();
                    }
                },
                None => {
                    log::warn!("{}", cosmic_apps::fan_control_install_hint());
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_fan_control_status(
                                "Not installed — see github.com/wiiznokes/fan-control".into(),
                            );
                        }
                    })
                    .ok();
                }
            }
        });
    });

    // cosmic-color-picker
    let state_cp = state.clone();
    let app_weak_cp = app.as_weak();
    app.on_launch_color_picker(move || {
        let state = state_cp.clone();
        let app_weak = app_weak_cp.clone();
        tokio::spawn(async move {
            let path = {
                let s = state.lock().await;
                s.cosmic_apps.color_picker_path.clone()
            };
            match path {
                Some(p) => match cosmic_apps::launch_color_picker(&p) {
                    Ok(()) => log::info!("cosmic-color-picker launched"),
                    Err(e) => {
                        log::error!("{}", e);
                        let msg = e;
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_color_picker_status(msg.into());
                            }
                        })
                        .ok();
                    }
                },
                None => {
                    log::warn!("{}", cosmic_apps::color_picker_install_hint());
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_color_picker_status(
                                "Not installed — see github.com/PixelDoted/cosmic-color-picker"
                                    .into(),
                            );
                        }
                    })
                    .ok();
                }
            }
        });
    });
}

// ─────────────────────────────── COSMIC themes callbacks ──────────────────

fn setup_cosmic_theme_callbacks(app: &MainWindow) {
    // Refresh themes from cosmic-themes.org
    let app_weak = app.as_weak();
    app.on_refresh_themes(move || {
        let app_weak = app_weak.clone();

        slint::invoke_from_event_loop({
            let app_weak = app_weak.clone();
            move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_themes_loading(true);
                    app.set_themes_error("".into());
                }
            }
        })
        .ok();

        tokio::spawn(async move {
            match cosmic_themes::fetch_themes().await {
                Ok(themes) => {
                    // Convert to Slint model structs
                    let slint_themes: Vec<_> = themes
                        .iter()
                        .map(|t| {
                            CosmicThemeData {
                                id: t.id as i32,
                                name: t.name.clone().into(),
                                author: t.author.clone().into(),
                                is_dark: t.is_dark,
                                accent_r: t.accent_r as i32,
                                accent_g: t.accent_g as i32,
                                accent_b: t.accent_b as i32,
                                bg_r: t.bg_r as i32,
                                bg_g: t.bg_g as i32,
                                bg_b: t.bg_b as i32,
                                downloads: t.downloads as i32,
                            }
                        })
                        .collect();

                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            let model = slint::ModelRc::new(slint::VecModel::from(slint_themes));
                            app.set_cosmic_themes(model);
                            app.set_themes_loading(false);
                            app.set_themes_error("".into());
                        }
                    })
                    .ok();
                }
                Err(e) => {
                    let msg = format!("Failed to fetch themes: {}", e);
                    log::error!("{}", msg);
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_themes_loading(false);
                            app.set_themes_error(msg.into());
                        }
                    })
                    .ok();
                }
            }
        });
    });

    // Install a theme by ID
    let app_weak = app.as_weak();
    app.on_install_theme(move |theme_id| {
        let app_weak = app_weak.clone();

        // Mark as installing
        slint::invoke_from_event_loop({
            let app_weak = app_weak.clone();
            move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_installing_theme_id(theme_id);
                }
            }
        })
        .ok();

        tokio::spawn(async move {
            // Look up the theme from the cached themes list
            // We need to re-fetch or use cache — use fetch_themes (will use cache if fresh)
            let theme_result = async {
                let themes = cosmic_themes::fetch_themes().await?;
                themes
                    .into_iter()
                    .find(|t| t.id == theme_id as u32)
                    .ok_or_else(|| {
                        Box::<dyn std::error::Error + Send + Sync>::from(
                            format!("Theme id {} not found", theme_id),
                        )
                    })
            }
            .await;

            match theme_result {
                Ok(theme) => {
                    match cosmic_themes::download_and_install_theme(&theme).await {
                        Ok(()) => {
                            let name = theme.name.clone();
                            show_toast(
                                &app_weak,
                                &format!(
                                    "Theme '{}' installed! Log out and back in to apply ✓",
                                    name
                                ),
                                true,
                            );
                        }
                        Err(e) => {
                            let msg = format!("Install failed: {}", e);
                            log::error!("{}", msg);
                            show_toast(&app_weak, &msg, false);
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("Could not find theme: {}", e);
                    log::error!("{}", msg);
                    show_toast(&app_weak, &msg, false);
                }
            }

            // Clear installing marker
            slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_installing_theme_id(-1);
                }
            })
            .ok();
        });
    });
}