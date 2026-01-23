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
}

#[derive(Clone, Default, Debug)]
pub enum PendingAction {
    #[default]
    None,
    ApplyTweaks,
    ResetDefaults,
    CleanSystem,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = MainWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::default()));
    
    // Load current system settings and update UI
    {
        let mut s = state.lock().await;
        s.tweaks = tweaks::load_current_settings().await;
        
        // Initialize backup state
        if backup::check_rclone_installed() {
            if let Ok(remotes) = backup::list_remotes() {
                if let Some(first) = remotes.first() {
                    s.backup.remote_name = Some(first.clone());
                    s.backup.is_configured = true;
                }
            }
        }
    }
    
    // Initialize UI with current settings
    initialize_ui(&app, &state).await;
    
    // Check tool dependencies
    app.set_sysbench_installed(benchmark::check_sysbench_installed());
    app.set_fio_installed(benchmark::check_fio_installed());
    
    // Set up callbacks
    setup_tweak_callbacks(&app, state.clone());
    setup_cleaner_callbacks(&app, state.clone());
    setup_backup_callbacks(&app, state.clone());
    setup_benchmark_callbacks(&app, state.clone());
    setup_sudo_callbacks(&app, state.clone());
    
    // Run the app
    app.run()?;
    
    Ok(())
}

async fn initialize_ui(app: &MainWindow, state: &Arc<Mutex<AppState>>) {
    let s = state.lock().await;
    let settings = &s.tweaks;
    
    // Set CPU governor index
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
    
    // I/O scheduler
    let io_index = match settings.io_scheduler.as_str() {
        "none" => 0,
        "mq-deadline" => 1,
        "kyber" => 2,
        _ => 1,
    };
    app.set_io_scheduler_index(io_index);
    app.set_trim_enabled(settings.trim_enabled);
    
    // GPU settings
    app.set_intel_guc_enabled(settings.intel_guc);
    app.set_intel_psr_enabled(settings.intel_psr);
    app.set_intel_fbc_enabled(settings.intel_fbc);
    
    // Power settings
    app.set_wifi_powersave(settings.wifi_powersave);
    app.set_audio_powersave(settings.audio_powersave);
    
    // Network settings
    app.set_tcp_fastopen(settings.tcp_fastopen as f32);
    app.set_tcp_low_latency(settings.tcp_low_latency);
    app.set_bbr_enabled(settings.bbr_enabled);
    
    // Kernel settings
    app.set_watchdog_enabled(settings.watchdog_enabled);
    let thp_index = match settings.transparent_hugepages.as_str() {
        "always" => 0,
        "madvise" => 1,
        "never" => 2,
        _ => 1,
    };
    app.set_thp_index(thp_index);
    
    // Backup status
    if let Some(remote) = &s.backup.remote_name {
        app.set_remote_name(remote.clone().into());
        app.set_is_configured(true);
    }
}

fn setup_tweak_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    // Apply Tweaks button
    let app_weak = app.as_weak();
    let state_clone = state.clone();
    
    app.on_apply_tweaks(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();
        
        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                // Collect current UI values into state
                let new_settings = tweaks::TweakSettings {
                    cpu_governor: match app.get_cpu_governor_index() {
                        0 => "powersave".into(),
                        1 => "schedutil".into(),
                        2 => "performance".into(),
                        _ => "schedutil".into(),
                    },
                    turbo_enabled: app.get_turbo_enabled(),
                    swappiness: app.get_swappiness() as u32,
                    vfs_cache_pressure: app.get_vfs_cache_pressure() as u32,
                    zram_enabled: app.get_zram_enabled(),
                    dirty_ratio: 20, // Default for now
                    dirty_background_ratio: 10,
                    io_scheduler: match app.get_io_scheduler_index() {
                        0 => "none".into(),
                        1 => "mq-deadline".into(),
                        2 => "kyber".into(),
                        _ => "mq-deadline".into(),
                    },
                    trim_enabled: app.get_trim_enabled(),
                    noatime: true, // Default
                    intel_guc: app.get_intel_guc_enabled(),
                    intel_psr: app.get_intel_psr_enabled(),
                    intel_fbc: app.get_intel_fbc_enabled(),
                    intel_rc6: true,
                    wifi_powersave: app.get_wifi_powersave(),
                    audio_powersave: app.get_audio_powersave(),
                    pcie_aspm: "default".into(),
                    tcp_fastopen: app.get_tcp_fastopen() as u32,
                    tcp_low_latency: app.get_tcp_low_latency(),
                    bbr_enabled: app.get_bbr_enabled(),
                    watchdog_enabled: app.get_watchdog_enabled(),
                    transparent_hugepages: match app.get_thp_index() {
                        0 => "always".into(),
                        1 => "madvise".into(),
                        2 => "never".into(),
                        _ => "madvise".into(),
                    },
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
    
    // Reset Defaults button
    let app_weak = app.as_weak();
    let state_clone = state.clone();
    
    app.on_reset_defaults(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();
        
        slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak.upgrade() {
                tokio::spawn(async move {
                    let mut s = state.lock().await;
                    s.pending_action = PendingAction::ResetDefaults;
                });
                
                app.set_sudo_action("Reset all settings to system defaults".into());
                app.set_show_sudo_dialog(true);
            }
        }).ok();
    });
    
    // Quick Profile button
    let app_weak = app.as_weak();
    
    app.on_set_profile(move |profile| {
        if let Some(app) = app_weak.upgrade() {
            match profile.as_str() {
                "battery" => {
                    app.set_cpu_governor_index(0); // powersave
                    app.set_turbo_enabled(false);
                    app.set_wifi_powersave(true);
                    app.set_audio_powersave(true);
                    app.set_intel_psr_enabled(true);
                    app.set_intel_fbc_enabled(true);
                }
                "balanced" => {
                    app.set_cpu_governor_index(1); // schedutil
                    app.set_turbo_enabled(true);
                    app.set_wifi_powersave(false);
                    app.set_audio_powersave(true);
                    app.set_intel_psr_enabled(true);
                    app.set_intel_fbc_enabled(true);
                }
                "performance" => {
                    app.set_cpu_governor_index(2); // performance
                    app.set_turbo_enabled(true);
                    app.set_wifi_powersave(false);
                    app.set_audio_powersave(false);
                    app.set_intel_psr_enabled(false);
                    app.set_intel_fbc_enabled(false);
                    app.set_intel_guc_enabled(true);
                }
                "presentation" => {
                    app.set_cpu_governor_index(1); // balanced
                    app.set_turbo_enabled(true);
                    app.set_intel_psr_enabled(false);
                }
                _ => {}
            }
        }
    });
}

fn setup_cleaner_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    // Scan button
    let app_weak = app.as_weak();
    let state_clone = state.clone();
    
    app.on_scan_cleaner(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();
        
        // Set scanning state
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
    
    // Clean Selected button
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
    
    // Find Duplicates button
    let app_weak = app.as_weak();
    
    app.on_find_duplicates(move || {
        let app_weak = app_weak.clone();
        
        tokio::spawn(async move {
            if let Some(home) = dirs::home_dir() {
                println!("Scanning {} for duplicates...", home.display());
                match cleaners::find_duplicates(&home).await {
                    Ok(groups) => {
                        let total_dups: usize = groups.iter().map(|g| g.len() - 1).sum();
                        slint::invoke_from_event_loop(move || {
                            if let Some(_app) = app_weak.upgrade() {
                                println!("Found {} duplicate file groups ({} files can be removed)", 
                                    groups.len(), total_dups);
                            }
                        }).ok();
                    }
                    Err(e) => {
                        eprintln!("Error scanning for duplicates: {}", e);
                    }
                }
            }
        });
    });
}

fn setup_backup_callbacks(app: &MainWindow, state: Arc<Mutex<AppState>>) {
    // Configure Rclone button
    app.on_configure_rclone(move || {
        if let Err(e) = backup::open_rclone_config() {
            eprintln!("Failed to open rclone config: {}", e);
        }
    });
    
    // Select Files button
    let app_weak = app.as_weak();
    let state_clone = state.clone();
    
    app.on_select_backup_files(move || {
        let app_weak = app_weak.clone();
        let state = state_clone.clone();
        
        // Use native file dialog via command
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
                    
                    slint::invoke_from_event_loop(move || {
                        if let Some(_app) = app_weak.upgrade() {
                            println!("Selected {} files for backup", file_count);
                        }
                    }).ok();
                }
            }
        });
    });
    
    // Start Sync button
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
                            }
                        }
                    }).ok();
                    
                    let app_weak_progress = app_weak.clone();
                    match backup::sync_to_remote(&remote, &files, |progress, status| {
                        let app_weak = app_weak_progress.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_transfer_progress(progress);
                                println!("{}", status);
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
                            eprintln!("Backup failed: {}", e);
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
        
        // Set running state
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
            let results = match bench_type.as_str() {
                "all" => benchmark::run_all().await,
                "cpu-single" => {
                    let score = benchmark::run_cpu_single().await;
                    let mut s = state.lock().await;
                    s.benchmark.cpu_single = score;
                    s.benchmark
                }
                "cpu-multi" => {
                    let score = benchmark::run_cpu_multi().await;
                    let mut s = state.lock().await;
                    s.benchmark.cpu_multi = score;
                    s.benchmark
                }
                "memory" => {
                    let score = benchmark::run_memory().await;
                    let mut s = state.lock().await;
                    s.benchmark.memory = score;
                    s.benchmark
                }
                "disk" => {
                    let (seq, rand) = benchmark::run_disk().await;
                    let mut s = state.lock().await;
                    s.benchmark.disk_sequential = seq;
                    s.benchmark.disk_random = rand;
                    s.benchmark
                }
                _ => benchmark::BenchmarkResults::default(),
            };
            
            // Store full results for "all"
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
        
        // Set loading state
        slint::invoke_from_event_loop({
            let app_weak = app_weak.clone();
            move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_sudo_loading(true);
                }
            }
        }).ok();
        
        tokio::spawn(async move {
            // Get pending action
            let pending = {
                let s = state.lock().await;
                s.pending_action.clone()
            };
            
            let result = match pending {
                PendingAction::ApplyTweaks => {
                    sudo_handler::authenticate_and_apply(&password, &state).await
                }
                PendingAction::ResetDefaults => {
                    sudo_handler::reset_to_defaults(&password).await
                }
                PendingAction::CleanSystem => {
                    sudo_handler::clean_system(&password, &state).await
                }
                PendingAction::None => Ok(()),
            };
            
            match result {
                Ok(()) => {
                    // Clear pending action
                    {
                        let mut s = state.lock().await;
                        s.pending_action = PendingAction::None;
                    }
                    
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_show_sudo_dialog(false);
                            app.set_sudo_password(slint::SharedString::new());
                            app.set_sudo_error(slint::SharedString::new());
                            app.set_sudo_loading(false);
                        }
                    }).ok();
                }
                Err(e) => {
                    let error_msg = e.to_string();
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
            app.set_sudo_password(slint::SharedString::new());
            app.set_sudo_error(slint::SharedString::new());
        }
    });
}
