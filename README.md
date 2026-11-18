<!-- Version: 3.0.0 · updated 26-11-18-02-45 -->
# sensors.gorilla 🦍

<!-- WHO-THIS-IS-FOR: managed block, do not edit by hand -->

**One live terminal readout of every hardware sensor your laptop will admit to, with nothing to install.**

Built for the people every other tool prices out: kids with no credit
card, 15-year-old laptops, data sold by the megabyte. Free forever, by
design, not as a trial.
Why, with the numbers: [PHILOSOPHY.md](https://github.com/gorillanobakaa-dot/Gorilla.Opencode/blob/main/PHILOSOPHY.md)

<!-- /WHO-THIS-IS-FOR -->

> *A zero-dependency, native compiled terminal hardware dashboard for
> **Windows (Generic + ThinkPad L15 Gen 3)**, **Linux (Sony VAIO SVE14 & x86_64)**,
> and **macOS (Intel & Apple Silicon)** — using full 24-bit TrueColor (16.7M colors)
> and reflowing instantaneously as you resize the terminal window.*

All operating systems and installers are published together on the single [Latest Release](https://github.com/gorillanobakaa-dot/sensors-gorilla/releases/latest) page.

---

## 🧸 Section A — For the Layman

### What does this do?

Imagine your laptop had a health panel — like a car dashboard — that told you everything happening inside it in real time: how hot every CPU core is, how fast the fan is spinning, how healthy the battery still is compared to brand new, how much life your SSD has left, and how long it will last under different usage styles.

Run one command in the terminal and you get a live, flicker-free, 24-bit TrueColor dashboard showing:

- 🌡️ **Temperatures** — CPU Package, every individual Core (including P-Cores & E-Cores on 12th Gen+ Intel), Motherboard / PCH Chipset, Integrated & Discrete GPU, NVMe/SATA SSDs, and ACPI thermal zones
- 💨 **Fan speed & cooling** — ThinkPad EC Fan RPM & BIOS mode (L15), Mac SMC Fan RPM, Sony VAIO fan %, plus active throttling flags
- ⚡ **Silicon power & clocks** — Live CPU Package / Core / Uncore-GPU wattage (RAPL) and live per-cluster boost frequencies
- 🔋 **Battery health** — Current charge %, voltage, live charge/discharge wattage, wear level % (actual capacity vs brand-new design), and charge cycle count
- 💾 **SSD health and lifetime** — Real-world years-to-go under Light, Developer Hardcore, and Enterprise workloads from live NVMe/SATA SMART wear & TBW counters
- 🧠 **Memory, GPU & Load** — RAM and pagefile/commit/swap usage with smooth TrueColor gradient bars, GPU VRAM & 3D/Video Decode/Encode utilization, active power plan

### Two Windows Editions

1. **Generic Windows (`sensors-gorilla.exe`)** — Works out of the box on any Windows 10 or Windows 11 PC (Intel, AMD, NVIDIA, laptop, or desktop). Zero drivers or Python needed.
2. **ThinkPad L15 Gen 3 Edition (`sensors-gorilla-l15.exe`)** — Tailored specifically for the Lenovo ThinkPad L15 Gen 3 (`21C3CTO1WW`, 12th Gen Intel Core i7-1255U 2P+8E hybrid CPU, Intel Iris Xe 96 EU, Micron 1TB PCIe Gen4 NVMe): separates P-core and E-core clocks/temps, reads the Lenovo Hardware Odometer, and (when run elevated with the signed PawnIO driver) reveals per-core die temps, RAPL watts, PCH temp, and ThinkPad EC Fan RPM read-only without any TPFanControl code.

---

## 💻 Section B — For the Developer

### Compiled Native Binaries (Zero On-the-Fly Compilation)

All four platform targets are compiled from **zero-dependency Rust (`std` only, empty `[dependencies]` in `Cargo.toml`)** with `opt-level = 3`, `lto = true`, `strip = true`:

| Target | Artifact | Notes |
|---|---|---|
| Windows Generic x64 | `sensors-gorilla-windows-generic-x86_64.exe` + `.installer.exe` | Static CRT (`+crt-static`), Win32 IOCTL / PDH / COM WMI / NVML / optional HWiNFO/LHM/PawnIO |
| Windows ThinkPad L15 Gen 3 | `sensors-gorilla-windows-thinkpad-l15-x86_64.exe` + `.installer.exe` | Static CRT, Alder Lake 2P+8E split, Lenovo Odometer, SHA-256 verified read-only PawnIO MSR/PCH/ThinkPad EC |
| Linux x86_64 | `sensors-gorilla-linux-x86_64` + `thermal-profile-daemon-linux-x86_64` | Static `x86_64-unknown-linux-musl` ELF, `hwmon`/`coretemp`/`k10temp`/`amdgpu`/RAPL/`smartctl`/`sony-laptop` |
| macOS (Intel & Apple Silicon) | `sensors-gorilla-macos-universal` + `sensors-gorilla-macos-x86_64` | Direct `IOKit` `AppleSMC` FFI + `ioreg` + `vm_stat` + `pmset` + Apple Terminal 256-color fallback |

### Windows Terminal 24-bit TrueColor & Instant Resize

- Enables `ENABLE_VIRTUAL_TERMINAL_PROCESSING` and UTF-8 (`CP_UTF8`) via `SetConsoleMode` / `SetConsoleOutputCP`.
- Uses synchronized output (`\x1b[?2026h` ... `\x1b[?2026l`) for tear-free live redraws.
- Continuous RGB interpolation (`\x1b[38;2;R;G;Bm`) across thermal, load, wear, and power scales plus gradient progress bars and separator rules.
- Wakes immediately on `WINDOW_BUFFER_SIZE_EVENT` (`WaitForSingleObject` on `STD_INPUT_HANDLE`) on Windows, and `SIGWINCH` self-pipe `poll()` on Linux/macOS.

### Usage

```powershell
sensors-gorilla            # Live 24-bit TrueColor dashboard (follows window resize)
sensors-gorilla --once     # One snapshot, then exit
sensors-gorilla --json     # JSON telemetry snapshot (for scripts & harnesses)
sensors-gorilla -n 5       # Refresh every 5 seconds
sensors-gorilla-l15 --no-ec  # L15 build skipping EC sideband probe
```

### Build Offline From Source

```bash
cargo test --release --offline --locked
cargo build --release --offline --locked
```

On Windows, verify and stage optional PawnIO 0.2.12 modules (checked against `pawnio/modules.sha256` before loading) and build Inno Setup installers:

```powershell
.\scripts\fetch_pawnio_modules.ps1
ISCC.exe installer\sensors-gorilla-generic.iss
ISCC.exe installer\sensors-gorilla-l15.iss
```
