# Tweakers ⚡

A high-performance system optimization and utility tool for Lilith Linux, built with Rust and Slint.

![Tweakers UI Preview](https://via.placeholder.com/1200x800.png?text=Tweakers+GUI+Preview)

## Lilith Linux Integration

Tweakers is an official system utility for **Lilith Linux**, providing comprehensive system optimization and maintenance tools.

### Part of Lilith Linux

| Component | Purpose |
|-----------|---------|
| **COSMIC Desktop** | Desktop Environment |
| **Tweakers** | System Optimization |
| **Shapeshifter** | Profile Manager |
| **Lilim** | AI Assistant |

## Features

### 🔧 System Tweaks
Comprehensive optimization categorized by component:
- **CPU**: Governor control, Turbo Boost toggles, and frequency scaling.
- **Memory**: Swappiness, VFS cache pressure, and ZRAM management.
- **Storage**: NVMe-specific I/O scheduler tuning and automatic TRIM.
- **GPU**: Intel GuC/HuC firmware loading, Framebuffer Compression (FBC), and Panel Self Refresh (PSR).
- **Network**: TCP Fast Open, Low Latency tuning, and BBR Congestion Control.
- **Kernel**: Watchdog toggles and Transparent Huge Pages (THP) control.
- **Profiles**: Quick-apply profiles for Battery Saver, Balanced, Performance, and Presentation.

### 🧹 System Cleaner
Safe and efficient cleaning of:
- APT Package Cache and orphans.
- Systemd Journal logs (vacuuming).
- User Thumbnail cache.
- Temporary files (/tmp and /var/tmp).
- Duplicate file finder (requires `fdupes`).

### ☁️ Backup (Rclone Integration)
- Easy configuration of cloud remotes via Rclone.
- Native file selection for manual backups.
- Real-time transfer progress tracking.

### 📊 Benchmark Suite
Test system performance before and after tweaks:
- CPU Single & Multi-thread performance.
- Memory bandwidth and latency.
- Disk Sequential and Random I/O (requires `sysbench` and `fio`).

## Installation

### Lilith Linux (Recommended)

Tweakers comes pre-installed on Lilith Linux. To reinstall or update:

```bash
sudo apt update
sudo apt install lilith-tweakers
```

### From Source

### Prerequisites
- Rust (latest stable)
- Slint dependencies
- Sudo privileges

```bash
# Ubuntu dependencies
sudo apt update
sudo apt install rclone sysbench fio fdupes zenity
```

### Build
```bash
cargo build --release
```

## Usage
Run the application with sudo privileges where necessary (tweak application and system cleaning):
```bash
cargo run
```

## Configuration
System tweaks are applied by creating configurations in:
- `/etc/sysctl.d/99-tweakers.conf`
- `/etc/modprobe.d/tweakers-i915.conf`

## License
MIT
