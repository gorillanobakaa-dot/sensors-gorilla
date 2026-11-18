# v3.0.1, for developers

## Scope

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
