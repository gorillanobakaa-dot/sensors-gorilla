# v3.0.2, in plain language

## What changed for you

- **Starts elevated automatically with UAC.** Both Windows executables now embed a `requireAdministrator` manifest, so double-clicking or launching them pops up the Windows UAC prompt and runs elevated right away without needing right-click -> "Run as administrator".
- **Standalone `.exe` reveals CPU core temperatures, CPU wattage, chipset temperature, and fan speed immediately.** Signed read-only PawnIO sensor modules are compiled inside the `.exe` (verified with SHA-256 before loading), `PawnIO` starts automatically if stopped, and ThinkPad EC automatically falls back from `0x1600/0x1604` to standard ACPI EC `0x62/0x66`.
- **Full Windows sensor & hardware exposure.** Adds Windows PDH `Energy Meter` RAPL (`PKG` / `PP0` Cores / `PP1` Iris Xe / `DRAM`), `Power Meter` total system draw, CPU `C1/C2/C3` deep sleep percentages, memory DIMM layout (`2x32 GB SK Hynix DDR4-3200`), panel resolution/Hz/brightness, active Wi-Fi/Ethernet link speed and RX/TX KB/s, and enumerated platform devices (`IPF SEN1..SEN6`, `Intel PMT`, `Lenovo ITS`, `TPM 2.0`, `Quectel LTE/GNSS`, cameras, biometric, audio).
- **No more flashing while running.** Updates lines smoothly in place with cursor hidden — matching the Linux terminal experience on both classic Windows Console (`conhost.exe`) and Windows Terminal.

## Two Windows editions and all OSes in one place

- **Generic Windows (`sensors-gorilla`)**: works on any Windows 10/11 Intel or AMD PC.
- **ThinkPad L15 Gen 3 (`sensors-gorilla-l15`)**: tailored for the 12th Gen i7-1255U (2 P-cores + 8 E-cores), Intel Iris Xe, Micron 1 TB NVMe, Lenovo Odometer, and read-only ThinkPad EC Fan RPM + board sensors (zero TPFanControl code).
- **Linux and macOS**: precompiled native binaries with zero on-the-fly Python compilation and crisp >256 px Gorilla icons.
