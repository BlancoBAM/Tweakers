/// resource_monitor.rs — Live system resource polling via /proc
///
/// Spawns a background Tokio task that reads /proc/stat and /proc/meminfo
/// every 2 seconds and pushes CPU% / RAM% updates to the Slint UI.

use std::time::Duration;
use tokio::time::sleep;

// ─────────────────────────────── CPU ──────────────────────────────────────

#[derive(Clone, Default)]
struct CpuStat {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CpuStat {
    fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }

    fn idle_total(&self) -> u64 {
        self.idle + self.iowait
    }
}

async fn read_cpu_stat() -> Option<CpuStat> {
    let content = tokio::fs::read_to_string("/proc/stat").await.ok()?;
    let first = content.lines().next()?;
    // "cpu  user nice system idle iowait irq softirq steal ..."
    let mut parts = first.split_whitespace().skip(1);
    Some(CpuStat {
        user: parts.next()?.parse().ok()?,
        nice: parts.next()?.parse().ok()?,
        system: parts.next()?.parse().ok()?,
        idle: parts.next()?.parse().ok()?,
        iowait: parts.next()?.parse().ok()?,
        irq: parts.next()?.parse().ok()?,
        softirq: parts.next()?.parse().ok()?,
        steal: parts.next()?.parse().ok()?,
    })
}

fn compute_cpu_percent(prev: &CpuStat, curr: &CpuStat) -> f32 {
    let total_diff = curr.total().saturating_sub(prev.total()) as f32;
    let idle_diff = curr.idle_total().saturating_sub(prev.idle_total()) as f32;
    if total_diff == 0.0 {
        return 0.0;
    }
    ((total_diff - idle_diff) / total_diff * 100.0).clamp(0.0, 100.0)
}

// ─────────────────────────────── RAM ──────────────────────────────────────

#[derive(Clone, Default)]
pub struct RamInfo {
    pub total_mb: u64,
    pub used_mb: u64,
    pub percent: f32,
}

async fn read_meminfo() -> RamInfo {
    let content = match tokio::fs::read_to_string("/proc/meminfo").await {
        Ok(c) => c,
        Err(_) => return RamInfo::default(),
    };

    let mut total_kb: u64 = 0;
    let mut available_kb: u64 = 0;

    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = parse_meminfo_kb(line);
        } else if line.starts_with("MemAvailable:") {
            available_kb = parse_meminfo_kb(line);
        }
    }

    if total_kb == 0 {
        return RamInfo::default();
    }

    let used_kb = total_kb.saturating_sub(available_kb);
    RamInfo {
        total_mb: total_kb / 1024,
        used_mb: used_kb / 1024,
        percent: (used_kb as f32 / total_kb as f32 * 100.0).clamp(0.0, 100.0),
    }
}

fn parse_meminfo_kb(line: &str) -> u64 {
    // Format: "MemTotal:       16384000 kB"
    line.split_whitespace()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// ─────────────────────────────── Monitor Task ─────────────────────────────

/// Start the background resource monitor. Call once from main() after the
/// Slint window is created. The task runs indefinitely until the process exits.
pub fn start_monitor_task(app_weak: slint::Weak<crate::MainWindow>) {
    tokio::spawn(async move {
        // Prime the first CPU sample — we need two readings to compute a delta.
        let mut prev_cpu = read_cpu_stat().await.unwrap_or_default();
        // Short initial delay so the first reading isn't immediately 0%
        sleep(Duration::from_millis(500)).await;

        loop {
            sleep(Duration::from_secs(2)).await;

            let curr_cpu = read_cpu_stat().await.unwrap_or_default();
            let cpu_pct = compute_cpu_percent(&prev_cpu, &curr_cpu);
            prev_cpu = curr_cpu;

            let ram = read_meminfo().await;

            let ram_pct = ram.percent;
            let ram_used = ram.used_mb as i32;
            let ram_total = ram.total_mb as i32;

            let app_weak2 = app_weak.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak2.upgrade() {
                    app.set_cpu_usage(cpu_pct);
                    app.set_ram_usage_pct(ram_pct);
                    app.set_ram_used_mb(ram_used);
                    app.set_ram_total_mb(ram_total);
                }
            })
            .ok();
        }
    });
}
