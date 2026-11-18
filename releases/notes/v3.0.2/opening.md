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
