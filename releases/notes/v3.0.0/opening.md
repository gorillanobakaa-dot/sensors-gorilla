## What this is

`sensors.gorilla` v3.0.0 — zero-dependency **compiled native** live terminal hardware sensor dashboard for **Windows (Generic x64 + ThinkPad L15 Gen 3), Linux (x86_64 static binary), and macOS (Intel & Apple Silicon)**. Every installer, executable, checksum, and icon for every operating system is on this single Latest release page.

## Should you download it?

Yes. Pick the file for your computer:

| Your computer | Download | What you get |
|---|---|---|
| Windows 10 / 11 (Generic x64) | `sensors-gorilla-3.0.0-win64-generic-installer.exe` (or `sensors-gorilla-windows-generic-x86_64.exe`) | Any Windows PC: 24-bit TrueColor, NVMe/SATA SMART, Battery IOCTL, PDH Thermal Zones, GPU VRAM/engines, CPU clocks |
| Windows 11 (ThinkPad L15 Gen 3) | `sensors-gorilla-3.0.0-win64-thinkpad-l15-installer.exe` (or `sensors-gorilla-windows-thinkpad-l15-x86_64.exe`) | Tailored for ThinkPad L15 Gen 3 (`21C3CTO1WW`): 2P+8E core split, Iris Xe, Micron NVMe, Lenovo Odometer, read-only PawnIO MSR/PCH/EC Fan RPM |
| Linux (x86_64 static binary) | `sensors-gorilla-linux-x86_64` (+ `thermal-profile-daemon-linux-x86_64`, `recreate_success.sh`, `sensors-gorilla-512.png`) | Static musl ELF binary + 512x512 desktop icon |
| macOS (Intel & Apple Silicon) | `sensors-gorilla-macos-universal` + `sensors-gorilla-macos-app.zip` (or `sensors-gorilla-macos-x86_64`) | Native Mach-O binary calling AppleSMC via IOKit + `.app` bundle with 1024x1024 `.icns` icon |

## Why this matters

1. **Compiled for every OS — nothing compiles on the fly on startup.** Windows, Linux, and macOS executables are compiled native machine code with zero Python runtime compilation overhead.
2. **Two Windows editions** (Generic Windows + ThinkPad L15 Gen 3), built with zero TPFanControl code, using Windows Terminal's full 24-bit TrueColor (16.7 million colors) and instant window-resize reflow.
3. **Crisp high-resolution Gorilla icons (>256 px)** embedded into every Windows `.exe`, installer, and shortcut, Linux `hicolor` 512x512 icon, and macOS `.icns`/`.app` bundle.

| File | Bytes | SHA-256 |
|---|---|---|
| `(filled in by release workflow from SHA256SUMS)` | — | — |
