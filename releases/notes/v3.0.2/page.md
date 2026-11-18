## What this is

`sensors.gorilla` v3.0.2 — zero-dependency **compiled native** live terminal hardware sensor dashboard for **Windows (Generic x64 + ThinkPad L15 Gen 3), Linux (x86_64 static binary), and macOS (Intel & Apple Silicon)**. Every installer, executable, checksum, and icon for every operating system is on this single Latest release page.

## Should you download it?

Yes. Pick the file for your computer:

| Your computer | Download | What you get |
|---|---|---|
| Windows 10 / 11 (Generic x64) | `sensors-gorilla-3.0.2-win64-generic-installer.exe` (or `sensors-gorilla-windows-generic-x86_64.exe`) | Any Windows PC: 24-bit TrueColor, zero-flicker redraw, NVMe/SATA SMART, Battery IOCTL, PDH Thermal Zones, GPU VRAM/engines, CPU clocks, embedded PawnIO Intel/AMD modules |
| Windows 11 (ThinkPad L15 Gen 3) | `sensors-gorilla-3.0.2-win64-thinkpad-l15-installer.exe` (or `sensors-gorilla-windows-thinkpad-l15-x86_64.exe`) | Tailored for ThinkPad L15 Gen 3 (`21C3CTO1WW`): 2P+8E core split, Iris Xe, Micron NVMe, Lenovo Odometer, embedded read-only PawnIO MSR/PCH/EC Fan RPM |
| Linux (x86_64 static binary) | `sensors-gorilla-linux-x86_64` (+ `thermal-profile-daemon-linux-x86_64`, `recreate_success.sh`, `sensors-gorilla-512.png`) | Static musl ELF binary + 512x512 desktop icon |
| macOS (Intel & Apple Silicon) | `sensors-gorilla-macos-universal` + `sensors-gorilla-macos-app.zip` (or `sensors-gorilla-macos-x86_64`) | Native Mach-O binary calling AppleSMC via IOKit + `.app` bundle with 1024x1024 `.icns` icon |

## Why this matters

1. **Auto-elevates via UAC & embeds PawnIO modules inside every `.exe`.** Both Windows executables include a `requireAdministrator` manifest (so clicking them prompts UAC and starts elevated automatically) plus embedded SHA-256-verified PawnIO modules (`IntelMSR`, `IntelPCHThermal`, `AMDFamily17`, `LpcThinkpadEC`, `LpcACPIEC`), direct `DeviceIoControl`, and `StartServiceW("PawnIO")`.
2. **Every Windows sensor exposed.** Combines PawnIO silicon telemetry with native Windows PDH `Energy Meter` RAPL (`PKG`/`PP0`/`PP1`/`DRAM`), `Power Meter`, `C1/C2/C3` residency, PPM limit flags, elevated `MSAcpi_ThermalZoneTemperature` / `BatteryTemperature`, `Win32_PhysicalMemory` DIMMs, panel resolution/Hz/brightness, NIC speed/RX/TX, and active platform devices (`IPF SEN1..SEN6`, `Intel PMT`, `Lenovo ITS`, `TPM 2.0`, `LTE/GNSS`, cameras, audio).
3. **Zero-flicker in-place redraw on Windows Console and Windows Terminal.** Uses `\x1b[H` + per-line `\x1b[K` + hidden cursor `\x1b[?25l` in one single write instead of clearing the whole screen before drawing.
4. **Compiled for every OS with crisp >256 px Gorilla icons.**

| File | Bytes | SHA-256 |
|---|---|---|
| `(filled in by release workflow from SHA256SUMS)` | — | — |

---
# In plain language: everything in this release

This is the complete explanation, not a summary of one. Nothing below is behind a link.

<!-- plain-language track: in full, on this page -->

### What changed for you

- **Starts elevated automatically with UAC.** Both Windows executables now embed a `requireAdministrator` manifest, so double-clicking or launching them pops up the Windows UAC prompt and runs elevated right away without needing right-click -> "Run as administrator".
- **Standalone `.exe` reveals CPU core temperatures, CPU wattage, chipset temperature, and fan speed immediately.** Signed read-only PawnIO sensor modules are compiled inside the `.exe` (verified with SHA-256 before loading), `PawnIO` starts automatically if stopped, and ThinkPad EC automatically falls back from `0x1600/0x1604` to standard ACPI EC `0x62/0x66`.
- **Full Windows sensor & hardware exposure.** Adds Windows PDH `Energy Meter` RAPL (`PKG` / `PP0` Cores / `PP1` Iris Xe / `DRAM`), `Power Meter` total system draw, CPU `C1/C2/C3` deep sleep percentages, memory DIMM layout (`2x32 GB SK Hynix DDR4-3200`), panel resolution/Hz/brightness, active Wi-Fi/Ethernet link speed and RX/TX KB/s, and enumerated platform devices (`IPF SEN1..SEN6`, `Intel PMT`, `Lenovo ITS`, `TPM 2.0`, `Quectel LTE/GNSS`, cameras, biometric, audio).
- **No more flashing while running.** Updates lines smoothly in place with cursor hidden — matching the Linux terminal experience on both classic Windows Console (`conhost.exe`) and Windows Terminal.

### Two Windows editions and all OSes in one place

- **Generic Windows (`sensors-gorilla`)**: works on any Windows 10/11 Intel or AMD PC.
- **ThinkPad L15 Gen 3 (`sensors-gorilla-l15`)**: tailored for the 12th Gen i7-1255U (2 P-cores + 8 E-cores), Intel Iris Xe, Micron 1 TB NVMe, Lenovo Odometer, and read-only ThinkPad EC Fan RPM + board sensors (zero TPFanControl code).
- **Linux and macOS**: precompiled native binaries with zero on-the-fly Python compilation and crisp >256 px Gorilla icons.

---

# For developers: how it works, and how to check it

Written for someone who will audit, fork or change the code. It covers the same release as the plain-language part above; neither is a summary of the other.

<!-- developer track: in full, on this page -->

### Scope

`v3.0.1..HEAD`:

- Embedded SHA-256-verified PawnIO 0.2.12 modules (`IntelMSR.bin`, `IntelPCHThermal.bin`, `AMDFamily17.bin`, `LpcThinkpadEC.bin`, `LpcACPIEC.bin`) via `include_bytes!("../pawnio/...")` in `src/win_sensors.rs`, allowing standalone Windows `.exe` binaries to unlock MSR, PCH, AMD SMN, and read-only EC sensors from any directory without separate `.bin` files.
- Added `PawnConn::Direct` fallback (`\\.\GLOBALROOT\Device\PawnIO` / `\\.\PawnIO` `DeviceIoControl` IOCTLs `0xA1B22084` / `0xA1B22104`) when `PawnIOLib.dll` is absent, plus `ensure_pawnio_driver_started()` (`OpenSCManagerW` + `OpenServiceW("PawnIO")` + `StartServiceW`), plus `LpcACPIEC` (`0x62`/`0x66`) read-only `0x80` fallback when `LpcThinkpadEC` (`0x1600`/`0x1604`) returns unsupported.
- Eliminated Windows Console (`conhost.exe`) redraw flashing in `src/lib.rs`: hides cursor (`\x1b[?25l`), moves home (`\x1b[H`), appends `\x1b[K` per line, clears trailing lines (`\x1b[J`) only after the footer, and flushes one single buffer per frame (clearing `\x1b[2J` only on terminal dimension change).
- Embedded `assets/icons/sensors-gorilla.manifest` (`CREATEPROCESS_MANIFEST_RESOURCE_ID 1 RT_MANIFEST` in `assets/icons/app.rc`) with `<requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>` and `PerMonitorV2` DPI awareness into all Windows binary targets.
- Added native Windows PDH `\Energy Meter(*)\Power` (`RAPL_Package0_PKG`, `PP0`, `PP1`, `DRAM`), `\Power Meter(_Total)\Power`, `\Processor Information(_Total)\% C1/C2/C3 Time`, and `Performance Limit Flags` / `% Performance Limit` counters in `src/win_sensors.rs`, plus elevated `MSAcpi_ThermalZoneTemperature` merging, `BatteryTemperature`, `Win32_TemperatureProbe`, and `Win32_PnPEntity` platform hardware badge enumeration.
- Fixed ThinkPad EC probing in `probe_pawnio` to test actual read-only EC register reads on `LpcThinkpadEC` (`0x1600`/`0x1604`) before accepting, falling through to `LpcACPIEC` (`0x62`/`0x66`) when secondary EC ports return `0xFF`/no data, and allowed `PawnConn::open` to fall through to `PawnConn::Direct` when `PawnIOLib.dll` fails for non-permission errors.
- Enumerated `Win32_PhysicalMemory` DIMMs (`ram_spec`), `WmiMonitorBrightness` + display resolution/Hz (`display_mode`, `brightness_pct`), `Win32_PnPEntity` Sensor count (`has_wwan_gnss`), and PDH `\Network Interface(*)` RX/TX KB/s + link Mbps in `src/win_sensors.rs` and `src/win_dashboard.rs`.
- Added `.github/workflows/ci.yml` Windows `fetch_pawnio_modules.ps1` step before `cargo test` / `cargo build` so `include_bytes!` resolves in CI.
