# v3.0.2, in plain language

## What changed for you

- **Running the downloaded `.exe` as Administrator now reveals CPU core temperatures, CPU wattage, chipset temperature, and fan speed immediately.** In `v3.0.1`, the PawnIO sensor modules were only loaded from a folder next to the `.exe`, so running the standalone `.exe` from `Downloads` showed `N/A` even as Administrator. In `v3.0.2`, those signed read-only modules are compiled right inside the `.exe` (and checked with SHA-256 before loading), and the `PawnIO` driver is started automatically if it was stopped.
- **No more flashing while running.** Instead of wiping the whole screen blank every 2 seconds before drawing, the dashboard now updates lines smoothly in place with the cursor hidden — matching the Linux terminal experience on both classic Windows Console (`conhost.exe`) and Windows Terminal.
- **More ThinkPad L15 & Windows hardware details exposed.** Added memory DIMM count, size, vendor, and DDR speed (`2x32 GB SK Hynix DDR4-3200`), panel resolution, refresh rate, and brightness percentage (`1920x1080 @ 60Hz (69% bright)`), plus active network adapter link speed and live RX/TX throughput.
- **Better diagnostics.** If PawnIO is not installed or not run as Administrator, the dashboard prints the exact reason instead of a generic message.

## Two Windows editions and all OSes in one place

- **Generic Windows (`sensors-gorilla`)**: works on any Windows 10/11 Intel or AMD PC.
- **ThinkPad L15 Gen 3 (`sensors-gorilla-l15`)**: tailored for the 12th Gen i7-1255U (2 P-cores + 8 E-cores), Intel Iris Xe, Micron 1 TB NVMe, Lenovo Odometer, and read-only ThinkPad EC Fan RPM + board sensors (zero TPFanControl code).
- **Linux and macOS**: precompiled native binaries with zero on-the-fly Python compilation and crisp >256 px Gorilla icons.
