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

---
# In plain language: everything in this release

This is the complete explanation, not a summary of one. Nothing below is behind a link.

<!-- plain-language track: in full, on this page -->

### What changed for you

- **Double-clicking the desktop icon or `.exe` now pops up UAC automatically instead of failing with error `0x800702e4`.** In `v3.0.2`, putting `requireAdministrator` inside the EXE manifest caused Windows 11's Windows Terminal shortcut (`wt.exe`) to throw `[error 2147943140 (0x800702e4)] The requested operation requires elevation` unless you manually right-clicked -> *Run as administrator*. In `v3.0.3`, the app starts cleanly with an `asInvoker` manifest and immediately triggers Windows UAC via `ShellExecuteW("runas")` so clicking the icon asks for UAC, starts elevated, and unlocks all CPU/PCH/EC sensors automatically.
- **Direct shortcut launch.** Installer shortcuts now launch the `.exe` directly rather than wrapping `wt.exe`, eliminating the extra non-elevated terminal tab flash.
- **No more stacked/repeating banners when running live.** Uses the terminal alternate screen buffer (`\x1b[?1049h`), groups per-core temperatures cleanly so both columns stay side-by-side in standard 1080p terminal windows, filters spurious 1°C EC placeholders, and never scrolls past the bottom row.
- **Includes every `v3.0.2` improvement:** embedded PawnIO sensor modules for standalone runs, zero-flicker in-place redraw, Windows PDH Energy/Power Meter RAPL, C1/C2/C3 sleep states, DIMMs, display brightness/Hz, NIC speed/RX/TX, and enumerated platform devices.

---

# For developers: how it works, and how to check it

Written for someone who will audit, fork or change the code. It covers the same release as the plain-language part above; neither is a summary of the other.

<!-- developer track: in full, on this page -->

### Scope

`v3.0.2..HEAD`:

- Replaced `requireAdministrator` in `assets/icons/sensors-gorilla.manifest` with `asInvoker` (keeping `PerMonitorV2` DPI awareness), fixing `ERROR_ELEVATION_REQUIRED` (`0x800702e4`) when Windows Terminal spawns a console `.exe` via `CreateProcess`.
- Added native `is_elevated()` (`OpenProcessToken` + `GetTokenInformation(TokenElevation)`) and `relaunch_as_admin()` (`ShellExecuteW("runas", exe, "--elevated-child ...")`) in `src/win_ffi.rs` and `src/win_sensors.rs`, invoked at startup in `src/lib.rs` on interactive Windows TTY launches (`--no-elevate` / `--once` / `--json` / `--version` / `--help` bypass UAC).
- Updated Inno Setup `[Icons]` in `installer/sensors-gorilla-generic.iss` and `installer/sensors-gorilla-l15.iss` to point Desktop and Start Menu shortcuts directly at `{app}\{#AppExe}` rather than `wt.exe`.
- Fixed Windows live layout overflow/scrolling (`src/lib.rs`, `src/win_dashboard.rs`, `src/win_sensors.rs`): enabled VT alternate screen (`\x1b[?1049h`/`\x1b[?1049l`), clamped body lines to `term_rows - 1` with no trailing newline after the footer, grouped P/E core temperatures inline (`<= 78` left cols) and split platform hardware badges (`<= 92` right cols) so both columns fit side-by-side without stacking, and filtered ThinkPad EC temperatures `< 15°C`.
