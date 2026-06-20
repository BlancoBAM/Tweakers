<p align="center">
  <img src="assets/icons/tweakers-256.png" alt="Tweakers Logo" width="300" style="border-radius: 20px; box-shadow: 0 8px 24px rgba(255, 68, 68, 0.2);" />
</p>

<h1 align="center">Tweakers ⚡</h1>

<p align="center">
  <strong>The Ultimate High-Performance System Optimization & Utility Suite for Lilith Linux</strong>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-red.svg" alt="License: MIT"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Built%20with-Rust%201.75+-brown.svg" alt="Built with Rust"></a>
  <a href="https://slint.dev"><img src="https://img.shields.io/badge/UI-Slint%201.5-blue.svg" alt="Slint UI"></a>
  <a href="https://github.com/qarmin/czkawka"><img src="https://img.shields.io/badge/Powered%20By-Czkawka-orange.svg" alt="Powered By Czkawka"></a>
</p>

---

**Tweakers** is a system utility created **specifically for Lilith Linux**. Built from the ground up using **Rust** and **Slint**, it delivers blistering performance, a modern dark aesthetic with red/orange accents, and robust security. 

It provides an intuitive graphical interface for advanced kernel adjustments, quick-apply performance profiles, safe temporary file cleaners, automated cloud backups, and a full system benchmarking suite.

---

## 🔍 Core Integration: Powered by Czkawka & Krokiet

The **File Analysis** section in the System Cleaner is fully integrated with **[Czkawka](https://github.com/qarmin/czkawka)** (and its modern Slint GUI frontend, **[Krokiet](https://github.com/qarmin/czkawka/tree/master/krokiet)**) created by the talented Rafał Mikrut (`qarmin`). 

Tweakers transparently leverages Czkawka's state-of-the-art algorithms on the backend. When you launch a file analysis, Tweakers automatically detects your environment and dynamically dispatches the scan:
* **Krokiet (GUI Mode):** Launches the stunning Krokiet interface seamlessly for interactive cleaning and deep analysis.
* **Czkawka (CLI Mode):** Executes headless terminal-based scans directly in the background and populates the results directly within the Tweakers status bar.
* **Smart Installer:** If neither tool is present on your system, Tweakers provides a one-click cargo installer that downloads and sets up Krokiet in the background.

We want to extend our sincere credit and gratitude to the Czkawka/Krokiet project for their excellent, open-source file duplicate and cleaning engines that make this seamless integration possible!

---

## ⚡ Key Features

### 🔧 1. Advanced System Tweaks
Tune every corner of your Lilith Linux installation with fine-grained control:
* **CPU & Governor Control:** Toggle Turbo Boost and set scaling governors (`powersave`, `schedutil`, `performance`).
* **Memory Management:** Adjust `swappiness` levels, VFS cache pressure, and manage ZRAM compressed swap space.
* **Storage Optimization:** Configure NVMe I/O schedulers (`none`, `mq-deadline`, `kyber`) and schedule automatic TRIM timers.
* **Intel GPU Tuning:** Enable GuC/HuC firmware loading, Framebuffer Compression (FBC), and Panel Self Refresh (PSR).
* **Network Tuning:** Optimize TCP stack parameters including TCP Fast Open (0–3), TCP Low Latency, and BBR Congestion Control.
* **Kernel Customization:** Toggle system watchdogs and Transparent Huge Pages (THP).
* **Quick Profiles:** One-click presets for **🔋 Battery Saver**, **⚖️ Balanced**, **🚀 Performance**, and **📺 Presentation**.

### 🧹 2. Deep System Cleaner
Keep your storage lean and clean:
* Safe cleaning of the APT package cache and orphaned packages.
* Automated vacuuming of Systemd Journal logs to keep log sizes under control.
* Safe removal of temporary files (`/tmp` and `/var/tmp`), ignoring active session files.
* **11-Mode File Analysis Grid** powered by Czkawka:
  1. 📄 **Duplicate Files** - Locates exact byte-for-byte copies.
  2. 📭 **Empty Files** - Scans for zero-byte redundant files.
  3. 📂 **Empty Directories** - Identifies and purges empty folders.
  4. 🖼 **Similar Images** - Visual similarity matching (finds resized or edited duplicates).
  5. 🎬 **Similar Videos** - Employs video clip visual analysis.
  6. 🎵 **Same Music** - Audio signature matching for duplicate tracks.
  7. 🔗 **Invalid Symlinks** - Highlights broken soft links.
  8. 💔 **Broken Files** - Scans for corrupted archives, PDFs, and ZIPs.
  9. 🏷 **Bad Extensions** - Flags files with wrong MIME-type file extensions.
  10. 📦 **Big Files** - Lists the largest files consuming your disk space.
  11. 🗑 **Temporary Files** - Sweeps system junk and cache pools.

### ☁️ 3. Backup (Rclone Integration)
* Direct configuration portal for Rclone remotes.
* Integrated file/folder selection using native system file choose dialogs.
* Real-time upload and sync progress tracking to secure your critical data to cloud storage.

### 📊 4. System Benchmark Suite
Measure the real-world impact of your tweaks with the integrated benchmark utility:
* **CPU Tests:** Single & multi-core performance checks (via `sysbench`).
* **Memory Bandwidth:** High-speed RAM throughput benchmark (via `sysbench`).
* **Storage I/O:** Sequential and Random Read/Write disk latency and IOPS testing (via `fio`).

---

## 🚀 Installation & Requirements

### Lilith Linux (Official Release)
Tweakers is shipped pre-installed as a core system component on all Lilith Linux distributions. To manually update or reinstall it:
```bash
sudo apt update
sudo apt install lilith-tweakers
```

### Building From Source

#### 1. System Dependencies
Ensure you have the required runtime tools and build dependencies installed:
```bash
# Core Slint and optimization utilities
sudo apt update
sudo apt install build-essential pkg-config libslint-dev rclone sysbench fio zenity

# Optional (highly recommended for file duplicates and analysis)
cargo install krokiet --locked
```

#### 2. Build & Run
Compile the optimized release binary using Cargo:
```bash
# Clone the repository
git clone https://github.com/Lilith-Linux/Tweakers.git
cd Tweakers/Tweakers

# Build the release profile
cargo build --release

# Run the app
./target/release/tweakers
```

---

## 🔒 Security & Sudo Escalation

Since Tweakers applies root-level configuration modifications to your kernel (`/etc/sysctl.d/99-tweakers.conf`) and hardware parameters, it runs with secure privilege escalation. 
* Tweakers uses a local custom **Askpass Helper** helper script which safely prompts for credentials inside a beautiful, custom-designed UI dialog.
* Password credentials are processed only in-memory and are never leaked to CLI arguments or system logs.

---

## 📂 Project Structure

```text
tweakers/
├── Cargo.toml           # Rust package configuration
├── build.rs             # Slint UI compiler script
├── icon.svg             # Application vector icon
├── logo.jpg             # High-resolution application hero logo
├── tweakers.desktop     # Desktop launcher
├── README.md            # This documentation file
├── src/
│   ├── main.rs          # Main event loop and UI callback handlers
│   ├── czkawka.rs       # Czkawka CLI & Krokiet GUI subprocess integration
│   ├── sudo_handler.rs  # Secure authorization executor
│   ├── cosmic_apps.rs   # COSMIC applications launching helper
│   ├── cosmic_themes.rs # Fetch, download, and install COSMIC desktop themes
│   ├── resource_monitor.rs # Live hardware resources monitor (CPU% and RAM%)
│   ├── tweaks/          # Kernel & hardware optimization modules
│   ├── cleaners/        # Temporary directory scanners
│   ├── backup/          # Rclone cloud storage backup manager
│   └── benchmark/       # System performance evaluation suite
└── ui/
    ├── main.slint       # Core interface components & screens
    └── theme.slint      # Styling variables & dark/red theme system
```

---

<p align="center">
  <i>Created with ❤️ and engineered specifically for the <b>Lilith Linux</b> operating system.</i>
</p>