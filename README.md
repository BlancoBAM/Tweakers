# Tweakers ⚡

[![License: MIT](https://img.shields.io/badge/License-MIT-red.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Built%20with-Rust%201.70+-brown.svg)](https://www.rust-lang.org/)
[![Slint](https://img.shields.io/badge/UI-Slint%201.5-blue.svg)](https://slint.dev)

A high-performance system optimization and utility tool for **Lilith Linux**, built with Rust and Slint.

![Tweakers UI Preview](screenshot.png)

## Lilith Linux Integration

Tweakers is an official system utility for **Lilith Linux**, providing comprehensive system optimization and maintenance tools.

### Part of Lilith Linux

| Component     | Purpose              |
|---------------|----------------------|
| **COSMIC Desktop** | Desktop Environment |
| **Tweakers**       | System Optimization  |
| **Shapeshifter**   | Profile Manager      |
| **Lilim**          | AI Assistant         |

## Features

### 🔧 System Tweaks
Comprehensive optimization categorized by component:
- **CPU**: Governor control (powersave, schedutil, performance), Turbo Boost toggle, frequency scaling info.
- **Memory**: Swappiness (0–100), VFS cache pressure, dirty page ratios, ZRAM compressed swap management.
- **Storage**: NVMe I/O scheduler tuning (none, mq-deadline, kyber), fstrim timer, noatime mount option.
- **GPU**: Intel GuC/HuC firmware loading, Framebuffer Compression (FBC), Panel Self Refresh (PSR).
- **Network**: TCP Fast Open (0–3), TCP Low Latency, BBR Congestion Control.
- **Kernel**: Watchdog toggles, Transparent Huge Pages (always/madvise/never).
- **Profiles**: Quick-apply profiles for Battery Saver, Balanced, Performance, and Presentation.

### 🧹 System Cleaner
Safe and efficient cleaning of:
- APT Package Cache and orphaned packages
- Systemd Journal logs (vacuuming)
- User Thumbnail cache
- Temporary files (/tmp and /var/tmp) — safely skips in-use files
- Duplicate file finder (requires `fdupes`)

### ☁️ Backup (Rclone Integration)
- Easy configuration of cloud remotes via Rclone
- Native file selection for manual backups using Zenity
- Real-time transfer progress tracking

### 📊 Benchmark Suite
Test system performance before and after tweaks:
- CPU Single & Multi-thread performance (via sysbench)
- Memory bandwidth benchmark (via sysbench)
- Disk Sequential and Random I/O (via fio)

## Installation

### Lilith Linux (Recommended)

Tweakers comes pre-installed on Lilith Linux. To reinstall or update:

```bash
sudo apt update
sudo apt install lilith-tweakers
```

### From Source

#### Prerequisites
- Rust 1.70+ (latest stable recommended)
- Slint dependencies (libslint-dev)
- Sudo privileges
- Required tools: rclone, sysbench, fio, fdupes, zenity

```bash
# Ubuntu/Debian dependencies
sudo apt update
sudo apt install rclone sysbench fio fdupes zenity libslint-dev build-essential pkg-config

# Build and run
cargo build --release
sudo ./target/release/tweakers
```

## Usage

Run the application with appropriate privileges:

```bash
# From source
cargo run

# Installed binary
tweakers
```

> **Note**: Tweakers will prompt for your password via a secure dialog when administrative privileges are needed for system changes.

## Configuration

System tweaks are applied by creating configurations in:
- `/etc/sysctl.d/99-tweakers.conf` — kernel parameter tuning
- `/etc/modprobe.d/tweakers-i915.conf` — Intel GPU module options

## Security

Tweakers uses a secure askpass mechanism for authentication — passwords are never passed as command-line arguments and are handled through environment-based sudo authentication.

## Project Structure

```
tweakers/
├── Cargo.toml           # Rust package configuration
├── build.rs             # Build script for Slint UI compilation
├── icon.svg             # Application icon
├── tweakers.desktop     # Desktop entry file
├── com.lilithlinux.tweakers.metainfo.xml  # AppStream metadata
├── README.md            # This file
├── debian/              # Debian packaging files
│   ├── control
│   ├── postinst
│   └── prerm
├── src/
│   ├── main.rs          # Application entry point and event handlers
│   ├── sudo_handler.rs  # Secure privilege escalation
│   ├── tweaks/          # System tweak implementations
│   │   ├── mod.rs       # Core settings and configuration generation
│   │   ├── cpu.rs       # CPU frequency and governor controls
│   │   ├── memory.rs    # Memory and swap management
│   │   ├── storage.rs   # Disk I/O and filesystem optimization
│   │   ├── gpu.rs       # Intel GPU parameters
│   │   ├── power.rs     # Power management (WiFi, audio, PCIe)
│   │   ├── network.rs   # Network stack tuning
│   │   └── kernel.rs    # Kernel parameters (THP, watchdog)
│   ├── cleaners/        # System cleaning
│   │   └── mod.rs
│   ├── backup/          # Rclone backup integration
│   │   └── mod.rs
│   └── benchmark/       # Performance benchmarking
│       └── mod.rs
└── ui/
    ├── main.slint       # Main application UI layout
    └── theme.slint      # Dark theme with red/orange accents
```

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

---

*Built with ❤️ for Lilith Linux*