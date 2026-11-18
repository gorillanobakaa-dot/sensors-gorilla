## What this is

`sensors.gorilla` v3.0.1 — zero-dependency **compiled native** live terminal hardware sensor dashboard for **Windows (Generic x64 + ThinkPad L15 Gen 3), Linux (x86_64 static binary), and macOS (Intel & Apple Silicon)**. Every installer, executable, checksum, and icon for every operating system is on this single Latest release page.

## Should you download it?

Yes. Pick the file for your computer:

| Your computer | Download | What you get |
|---|---|---|
| Windows 10 / 11 (Generic x64) | `sensors-gorilla-3.0.1-win64-generic-installer.exe` (or `sensors-gorilla-windows-generic-x86_64.exe`) | Any Windows PC: 24-bit TrueColor, NVMe/SATA SMART, Battery IOCTL, PDH Thermal Zones, GPU VRAM/engines, CPU clocks |
| Windows 11 (ThinkPad L15 Gen 3) | `sensors-gorilla-3.0.1-win64-thinkpad-l15-installer.exe` (or `sensors-gorilla-windows-thinkpad-l15-x86_64.exe`) | Tailored for ThinkPad L15 Gen 3 (`21C3CTO1WW`): 2P+8E core split, Iris Xe, Micron NVMe, Lenovo Odometer, read-only PawnIO MSR/PCH/EC Fan RPM |
| Linux (x86_64 static binary) | `sensors-gorilla-linux-x86_64` (+ `thermal-profile-daemon-linux-x86_64`, `recreate_success.sh`, `sensors-gorilla-512.png`) | Static musl ELF binary + 512x512 desktop icon |
| macOS (Intel & Apple Silicon) | `sensors-gorilla-macos-universal` + `sensors-gorilla-macos-app.zip` (or `sensors-gorilla-macos-x86_64`) | Native Mach-O binary calling AppleSMC via IOKit + `.app` bundle with 1024x1024 `.icns` icon |

## Why this matters

1. **Compiled for every OS — nothing compiles on the fly on startup.** Windows, Linux, and macOS executables are compiled native machine code with zero Python runtime compilation overhead.
2. **Two Windows editions** (Generic Windows + ThinkPad L15 Gen 3), built with zero TPFanControl code, using Windows Terminal's full 24-bit TrueColor (16.7 million colors) and instant window-resize reflow.
3. **Crisp high-resolution Gorilla icons (>256 px)** embedded into every Windows `.exe`, installer, and shortcut, Linux `hicolor` 512x512 icon, and macOS `.icns`/`.app` bundle.

| File | Bytes | SHA-256 |
|---|---|---|
| `(filled in by release workflow from SHA256SUMS)` | — | — |

---
# In plain language: everything in this release

This is the complete explanation, not a summary of one. Nothing below is behind a link.

<!-- plain-language track: in full, on this page -->



---

# For developers: how it works, and how to check it

Written for someone who will audit, fork or change the code. It covers the same release as the plain-language part above; neither is a summary of the other.

<!-- developer track: in full, on this page -->

### Scope

`v2.2.0..origin/main`:

- Zero-dependency Rust workspace (`[dependencies]` empty in `Cargo.toml`, built `--offline --locked`, `opt-level = 3`, `lto = true`, `strip = true`, `+crt-static` on Windows, `x86_64-unknown-linux-musl` static ELF on Linux, universal `x86_64`+`aarch64` Mach-O on macOS).
- `sensors-gorilla` (Generic Windows / Linux / macOS), `sensors-gorilla-l15` (Windows ThinkPad L15 Gen 3), and `thermal-profile-daemon` (compiled native Linux daemon).
- Windows telemetry engine (`src/win_ffi.rs`, `src/win_wmi.rs`, `src/win_sensors.rs`, `src/win_dashboard.rs`):
  - `DeviceIoControl`: `IOCTL_STORAGE_QUERY_PROPERTY` (NVMe Log Page `0x02` SMART Health Log, `StorageDeviceProperty`, `StorageDeviceTemperatureProperty`), `IOCTL_BATTERY_QUERY_TAG` / `INFORMATION` / `STATUS`.
  - `PDH` persistent sampler: `\Thermal Zone Information(*)`, `\Processor Information(*)`, `\GPU Adapter Memory(*)`, `\GPU Engine(*)`, `\PhysicalDisk(_Total)`.
  - `CallNtPowerInformation(ProcessorInformation)`, `GetLogicalProcessorInformationEx` (hybrid `EfficiencyClass` P-core vs E-core), `PowerGetActiveScheme`, `GlobalMemoryStatusEx`, `K32GetPerformanceInfo`.
  - Direct COM `IWbemLocator` WMI queries (`root\CIMV2`, `root\WMI`, `root\Lenovo`, `root\Microsoft\Windows\Storage`, `root\LibreHardwareMonitor`, `root\OpenHardwareMonitor`) with zero PowerShell subprocesses.
  - Optional dynamic `nvml.dll`, `HKCU\Software\HWiNFO64\VSB`, and optional read-only `PawnIOLib.dll` (`IntelMSR.bin`, `IntelPCHThermal.bin`, `AMDFamily17.bin`, `LpcThinkpadEC.bin`) verified against pinned SHA-256 digests in `src/sha256.rs` before loading (fail closed). Zero TPFanControl code.
- Windows Terminal VT engine (`ENABLE_VIRTUAL_TERMINAL_PROCESSING`, `CP_UTF8`, `\x1b[?2026h` synchronized frames, `WINDOW_BUFFER_SIZE_EVENT` instant resize) + continuous 24-bit RGB gradients in `src/ansi.rs` (preserving 256-color fallback on legacy `Apple_Terminal`).
- High-resolution icons (`16, 32, 48, 256` px Win32 resource via `build.rs` + `assets/icons/app.rc` + Inno Setup installers; `256/512` px Linux `hicolor` + `.desktop`; `16..1024` px macOS `.icns` + `sensors.gorilla.app` bundle).
- Fixed Windows release artifact staging path in `.github/workflows/release.yml` (`dist/`).
