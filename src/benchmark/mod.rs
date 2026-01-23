use std::process::Command;
use regex::Regex;

#[derive(Clone, Default, Debug, Copy)]
pub struct BenchmarkResults {
    pub cpu_single: f64,   // events/sec
    pub cpu_multi: f64,    // events/sec
    pub memory: f64,       // MB/sec
    pub disk_sequential: f64, // MB/sec
    pub disk_random: f64,  // IOPS
}

/// Check if sysbench is installed
pub fn check_sysbench_installed() -> bool {
    which::which("sysbench").is_ok()
}

/// Check if fio is installed
pub fn check_fio_installed() -> bool {
    which::which("fio").is_ok()
}

/// Run all benchmarks
pub async fn run_all() -> BenchmarkResults {
    let cpu_single = run_cpu_single().await;
    let cpu_multi = run_cpu_multi().await;
    let memory = run_memory().await;
    let (disk_seq, disk_rand) = run_disk().await;
    
    BenchmarkResults {
        cpu_single,
        cpu_multi,
        memory,
        disk_sequential: disk_seq,
        disk_random: disk_rand,
    }
}

/// Run single-threaded CPU benchmark
pub async fn run_cpu_single() -> f64 {
    let output = Command::new("sysbench")
        .args(["cpu", "--threads=1", "--time=10", "run"])
        .output();
    
    parse_sysbench_cpu(output.ok())
}

/// Run multi-threaded CPU benchmark
pub async fn run_cpu_multi() -> f64 {
    // Get number of cores
    let num_cpus = num_cpus::get();
    
    let output = Command::new("sysbench")
        .args(["cpu", &format!("--threads={}", num_cpus), "--time=10", "run"])
        .output();
    
    parse_sysbench_cpu(output.ok())
}

/// Parse sysbench CPU output for events per second
fn parse_sysbench_cpu(output: Option<std::process::Output>) -> f64 {
    if let Some(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Look for "events per second:" line
        let re = Regex::new(r"events per second:\s+([\d.]+)").unwrap();
        if let Some(caps) = re.captures(&stdout) {
            if let Some(m) = caps.get(1) {
                return m.as_str().parse().unwrap_or(0.0);
            }
        }
    }
    0.0
}

/// Run memory bandwidth benchmark
pub async fn run_memory() -> f64 {
    let num_cpus = num_cpus::get();
    
    let output = Command::new("sysbench")
        .args([
            "memory",
            &format!("--threads={}", num_cpus),
            "--memory-block-size=1M",
            "--memory-total-size=10G",
            "run"
        ])
        .output();
    
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Look for "MiB/sec" in output
        let re = Regex::new(r"([\d.]+)\s+MiB/sec").unwrap();
        if let Some(caps) = re.captures(&stdout) {
            if let Some(m) = caps.get(1) {
                return m.as_str().parse().unwrap_or(0.0);
            }
        }
    }
    0.0
}

/// Run disk benchmarks (sequential and random read)
pub async fn run_disk() -> (f64, f64) {
    let seq = run_disk_sequential().await;
    let rand = run_disk_random().await;
    (seq, rand)
}

/// Sequential read benchmark using fio
async fn run_disk_sequential() -> f64 {
    let output = Command::new("fio")
        .args([
            "--name=seq_read",
            "--rw=read",
            "--bs=1M",
            "--size=1G",
            "--numjobs=1",
            "--time_based",
            "--runtime=10",
            "--output-format=json",
        ])
        .output();
    
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse JSON for read bandwidth
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(bw) = json["jobs"][0]["read"]["bw_mean"].as_f64() {
                return bw / 1000.0; // Convert KB/s to MB/s
            }
        }
    }
    0.0
}

/// Random read benchmark using fio
async fn run_disk_random() -> f64 {
    let output = Command::new("fio")
        .args([
            "--name=rand_read",
            "--rw=randread",
            "--bs=4k",
            "--size=1G",
            "--numjobs=1",
            "--time_based",
            "--runtime=10",
            "--output-format=json",
        ])
        .output();
    
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse JSON for read IOPS
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(iops) = json["jobs"][0]["read"]["iops_mean"].as_f64() {
                return iops;
            }
        }
    }
    0.0
}
