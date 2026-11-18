# v3.0.0, in plain language

This release turns `sensors.gorilla` into a fast, compiled program across **Windows, Linux, and macOS** — and puts all downloads for every operating system together on one page.

### What changed for you

- **Starts instantly — no compiling on the fly.** Previously, Linux and macOS ran a plain-text Python script that had to recompile itself every time you started it. Now Windows, Linux, and macOS all run precompiled native executables.
- **Two Windows versions:**
  - **Generic Windows (`sensors-gorilla`)**: works on any Windows 10 or 11 desktop or laptop right out of the box with no extra drivers. Shows CPU clocks and load, thermal zone temperatures, NVMe and SATA SSD health and years-of-life projections, battery health and cycle count, GPU memory and usage, and RAM/commit memory.
  - **ThinkPad L15 Gen 3 (`sensors-gorilla-l15`)**: tailored specifically for this laptop's 12th Gen Intel i7-1255U (2 Performance cores + 8 Efficiency cores), Intel Iris Xe graphics, Micron 1 TB NVMe SSD, and Lenovo Hardware Odometer. When run as Administrator with the signed PawnIO driver installed, it also reveals per-core die temperatures, CPU/GPU/package wattage, PCH chipset temperature, and ThinkPad EC fan speed (RPM) safely read-only — with zero TPFanControl code.
- **Full 24-bit TrueColor in Windows Terminal.** Smooth 16.7-million-color thermal gradients, gradient progress bars, gradient separator lines, and the Gorilla Skynet System fire-gradient banner that reflows the instant you resize or maximise the window.
- **Crisp Gorilla icons on every OS.** The 256x256 icon is embedded directly into the Windows programs, installers, and shortcuts; Linux gets 256x256 and 512x512 icons with a desktop launcher; macOS gets an `.app` bundle with a 1024x1024 `.icns` icon.

### How to install on Windows

Download either `sensors-gorilla-3.0.0-win64-generic-installer.exe` (for any Windows PC) or `sensors-gorilla-3.0.0-win64-thinkpad-l15-installer.exe` (for a ThinkPad L15 Gen 3), double-click to install, and run `sensors-gorilla` or `sensors-gorilla-l15` in Windows Terminal.
