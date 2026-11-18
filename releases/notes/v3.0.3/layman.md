# v3.0.3, in plain language

## What changed for you

- **Double-clicking the desktop icon or `.exe` now pops up UAC automatically instead of failing with error `0x800702e4`.** In `v3.0.2`, putting `requireAdministrator` inside the EXE manifest caused Windows 11's Windows Terminal shortcut (`wt.exe`) to throw `[error 2147943140 (0x800702e4)] The requested operation requires elevation` unless you manually right-clicked -> *Run as administrator*. In `v3.0.3`, the app starts cleanly with an `asInvoker` manifest and immediately triggers Windows UAC via `ShellExecuteW("runas")` so clicking the icon asks for UAC, starts elevated, and unlocks all CPU/PCH/EC sensors automatically.
- **Direct shortcut launch.** Installer shortcuts now launch the `.exe` directly rather than wrapping `wt.exe`, eliminating the extra non-elevated terminal tab flash.
- **No more stacked/repeating banners when running live.** Uses the terminal alternate screen buffer (`\x1b[?1049h`), groups per-core temperatures cleanly so both columns stay side-by-side in standard 1080p terminal windows, filters spurious 1°C EC placeholders, and never scrolls past the bottom row.
- **Includes every `v3.0.2` improvement:** embedded PawnIO sensor modules for standalone runs, zero-flicker in-place redraw, Windows PDH Energy/Power Meter RAPL, C1/C2/C3 sleep states, DIMMs, display brightness/Hz, NIC speed/RX/TX, and enumerated platform devices.
