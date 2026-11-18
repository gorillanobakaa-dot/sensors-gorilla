## What this is

`sensors.gorilla` v3.0.3 — zero-dependency **compiled native** live terminal hardware sensor dashboard for **Windows (Generic x64 + ThinkPad L15 Gen 3), Linux (x86_64 static binary), and macOS (Intel & Apple Silicon)**. Every installer, executable, checksum, and icon for every operating system is on this single Latest release page.

## Should you download it?

Yes. Pick the file for your computer:

| Your computer | Download | What you get |
|---|---|---|
| Windows 10 / 11 (Generic x64) | `sensors-gorilla-3.0.3-win64-generic-installer.exe` (or `sensors-gorilla-windows-generic-x86_64.exe`) | Any Windows PC: double-click prompts UAC cleanly (zero `0x800702e4` Windows Terminal error), 24-bit TrueColor, zero-flicker redraw, NVMe/SATA SMART, Battery IOCTL, PDH Energy/Power Meter RAPL, embedded PawnIO modules |
| Windows 11 (ThinkPad L15 Gen 3) | `sensors-gorilla-3.0.3-win64-thinkpad-l15-installer.exe` (or `sensors-gorilla-windows-thinkpad-l15-x86_64.exe`) | Tailored for ThinkPad L15 Gen 3 (`21C3CTO1WW`): auto-elevates via UAC on double-click, 2P+8E core split, Iris Xe, Micron NVMe, Lenovo Odometer, embedded read-only PawnIO MSR/PCH/EC Fan RPM |
| Linux (x86_64 static binary) | `sensors-gorilla-linux-x86_64` (+ `thermal-profile-daemon-linux-x86_64`, `recreate_success.sh`, `sensors-gorilla-512.png`) | Static musl ELF binary + 512x512 desktop icon |
| macOS (Intel & Apple Silicon) | `sensors-gorilla-macos-universal` + `sensors-gorilla-macos-app.zip` (or `sensors-gorilla-macos-x86_64`) | Native Mach-O binary calling AppleSMC via IOKit + `.app` bundle with 1024x1024 `.icns` icon |

## Why this matters

1. **Fixes `0x800702e4` when double-clicking Desktop shortcuts or running inside Windows Terminal.** Uses an `asInvoker` manifest so Windows Terminal / Explorer starts the executable cleanly, then calls native Win32 `ShellExecuteW("runas")` at startup to show the UAC elevation prompt automatically (without needing right-click -> Run as administrator).
2. **All Windows, Linux, and macOS sensors in one place.** Embedded SHA-256-verified PawnIO modules (`IntelMSR`, `IntelPCHThermal`, `AMDFamily17`, `LpcThinkpadEC`, `LpcACPIEC`), PDH Energy Meter RAPL (`PKG`/`PP0`/`PP1`/`DRAM`), Power Meter, C-states, `Win32_PhysicalMemory` DIMMs, display brightness/mode, network link/throughput, and zero TPFanControl code.
3. **Zero-flicker redraw & crisp >256 px Gorilla icons.**

| File | Bytes | SHA-256 |
|---|---|---|
| `(filled in by release workflow from SHA256SUMS)` | — | — |
