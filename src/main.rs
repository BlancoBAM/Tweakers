mod tweaks;
mod cleaners;
mod backup;
mod benchmark;
mod sudo_handler;

use std::sync::Arc;
use std::path::PathBuf;
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
}

#[derive(Clone, Default, Debug, PartialEq)]
pub enum PendingAction {
    #[default]
    None,
    ApplyTweaks,
    ResetDefaults,
    CleanSystem,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let app = MainWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::default()));

    {
        let mut s = state.lock().await;
        s.tweaks = tweaks::load_current_settings().await;
        s.askpass_path = String::new();

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

    app.run()?;

    Ok(())
}

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
}

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
        }).ok();
    });

    let app_weak = app.as_weak();
    let state_clone2 = state.clone();
    let _state_clone = state.clone();

    app.on_reset_defaults(move || {
        let app_weak = app_weak.clone();

        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_sudo_action("Reset all settings to system defaults".into());
                app.set_show_sudo_dialog(true);
            }
        }).ok();
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
            }).ok();
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
        }).ok();
    });

    let app_weak2 = app.as_weak();

    app.on_find_duplicates(move || {
        let app_weak3 = app_weak2.clone();

        tokio::spawn(async move {
            if let Some(home) = dirs::home_dir() {
                match cleaners::find_duplicates(&home).await {
                    Ok(groups) => {
                        let total_dups: usize = groups.iter().map(|g| g.len().saturating_sub(1)).sum();
                        log::info!("Found {} duplicate file groups ({} files can be removed)", groups.len(), total_dups);
                        if let Some(app) = app_weak3.upgrade() {
                            app.set_apt_cache_size(0.0);
                        }
                    }
                    Err(e) => {
                        log::error!("Duplicate scan failed: {}", e);
                    }
                }
            }
        });
    });
}

fn setup_backup_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    app.on_configure_rclone(move || {
        if let Err(e) = backup::open_rclone_config() {
            log::error!("Failed to open rclone config: {}", e);
        }
    });

    let app_weak2 = app.as_weak();
    let state_clone = state.clone();

    app.on_select_backup_files(move || {
        let app_weak = app_weak2.clone();
        let state = state_clone.clone();

        tokio::spawn(async move {
            let output = std::process::Command::new("zenity")
                .args(["--file-selection", "--multiple", "--separator=\n", "--title=Select files to backup"])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .map(PathBuf::from)
                        .collect();

                    let file_count = files.len();

                    {
                        let mut s = state.lock().await;
                        s.backup.queued_files = files;
                    }

                    log::info!("Selected {} files for backup", file_count);
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
                    }).ok();

                    let app_weak_progress = app_weak.clone();
                    match backup::sync_to_remote(&remote, &files, |progress, _status| {
                        let app_weak = app_weak_progress.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_transfer_progress(progress);
                            }
                        }).ok();
                    }).await {
                        Ok(()) => {
                            slint::invoke_from_event_loop(move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_is_transferring(false);
                                    app.set_transfer_progress(1.0);
                                }
                            }).ok();
                        }
                        Err(e) => {
                            log::error!("Backup failed: {}", e);
                            slint::invoke_from_event_loop(move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_is_transferring(false);
                                }
                            }).ok();
                        }
                    }
                }
            }
        });
    });
}

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
        }).ok();

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
            }).ok();
        });
    });
}

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
        }).ok();

        tokio::spawn(async move {
            let askpass_content = format!("#!/bin/sh\necho '{}'\n", password);
            let askpass_path = format!("/tmp/tweakers-askpass-{}", std::process::id());

            if let Err(e) = tokio::fs::write(&askpass_path, &askpass_content).await {
                slint::invoke_from_event_loop({
                    let app_weak = app_weak.clone();
                    move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_sudo_loading(false);
                            app.set_sudo_error(format!("Failed to create askpass helper: {}", e).into());
                        }
                    }
                }).ok();
                return;
            }

            let _ = tokio::process::Command::new("chmod")
                .args(["700", &askpass_path])
                .output()
                .await;

            let verify_output = tokio::process::Command::new("bash")
                .args(["-c", &format!("SUDO_ASKPASS={} sudo -A -n true 2>&1", askpass_path)])
                .env("SUDO_ASKPASS", &askpass_path)
                .env("DISPLAY", ":0")
                .output()
                .await;

            if !verify_output.map(|o| o.status.success()).unwrap_or(false) {
                let _ = tokio::fs::remove_file(&askpass_path).await;
                slint::invoke_from_event_loop({
                    let app_weak = app_weak.clone();
                    move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_sudo_loading(false);
                            app.set_sudo_error("Incorrect password or no sudo privileges".into());
                        }
                    }
                }).ok();
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
                    {
                        let mut s = state.lock().await;
                        s.pending_action = PendingAction::None;
                    }

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
                                            app.set_thumbnail_cache_size(sizes.thumbnail_cache_mb as f32);
                                            app.set_journal_size(sizes.journal_mb as f32);
                                            app.set_temp_size(sizes.temp_mb as f32);
                                            app.set_orphan_packages(sizes.orphan_count as i32);
                                        }
                                    }).ok();
                                });
                            }
                        }
                    }).ok();
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    log::error!("Operation failed: {}", error_msg);
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_sudo_error(error_msg.into());
                            app.set_sudo_loading(false);
                        }
                    }).ok();
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