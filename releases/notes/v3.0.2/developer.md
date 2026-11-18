# v3.0.2, for developers

## Scope

`v3.0.1..HEAD`:

- Embedded SHA-256-verified PawnIO 0.2.12 modules (`IntelMSR.bin`, `IntelPCHThermal.bin`, `AMDFamily17.bin`, `LpcThinkpadEC.bin`, `LpcACPIEC.bin`) via `include_bytes!("../pawnio/...")` in `src/win_sensors.rs`, allowing standalone Windows `.exe` binaries to unlock MSR, PCH, AMD SMN, and read-only EC sensors from any directory without separate `.bin` files.
- Added `PawnConn::Direct` fallback (`\\.\GLOBALROOT\Device\PawnIO` / `\\.\PawnIO` `DeviceIoControl` IOCTLs `0xA1B22084` / `0xA1B22104`) when `PawnIOLib.dll` is absent, plus `ensure_pawnio_driver_started()` (`OpenSCManagerW` + `OpenServiceW("PawnIO")` + `StartServiceW`), plus `LpcACPIEC` (`0x62`/`0x66`) read-only `0x80` fallback when `LpcThinkpadEC` (`0x1600`/`0x1604`) returns unsupported.
- Eliminated Windows Console (`conhost.exe`) redraw flashing in `src/lib.rs`: hides cursor (`\x1b[?25l`), moves home (`\x1b[H`), appends `\x1b[K` per line, clears trailing lines (`\x1b[J`) only after the footer, and flushes one single buffer per frame (clearing `\x1b[2J` only on terminal dimension change).
- Embedded `assets/icons/sensors-gorilla.manifest` (`CREATEPROCESS_MANIFEST_RESOURCE_ID 1 RT_MANIFEST` in `assets/icons/app.rc`) with `<requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>` and `PerMonitorV2` DPI awareness into all Windows binary targets.
- Added native Windows PDH `\Energy Meter(*)\Power` (`RAPL_Package0_PKG`, `PP0`, `PP1`, `DRAM`), `\Power Meter(_Total)\Power`, `\Processor Information(_Total)\% C1/C2/C3 Time`, and `Performance Limit Flags` / `% Performance Limit` counters in `src/win_sensors.rs`, plus elevated `MSAcpi_ThermalZoneTemperature` merging, `BatteryTemperature`, `Win32_TemperatureProbe`, and `Win32_PnPEntity` platform hardware badge enumeration.
- Fixed ThinkPad EC probing in `probe_pawnio` to test actual read-only EC register reads on `LpcThinkpadEC` (`0x1600`/`0x1604`) before accepting, falling through to `LpcACPIEC` (`0x62`/`0x66`) when secondary EC ports return `0xFF`/no data, and allowed `PawnConn::open` to fall through to `PawnConn::Direct` when `PawnIOLib.dll` fails for non-permission errors.
- Enumerated `Win32_PhysicalMemory` DIMMs (`ram_spec`), `WmiMonitorBrightness` + display resolution/Hz (`display_mode`, `brightness_pct`), `Win32_PnPEntity` Sensor count (`has_wwan_gnss`), and PDH `\Network Interface(*)` RX/TX KB/s + link Mbps in `src/win_sensors.rs` and `src/win_dashboard.rs`.
- Added `.github/workflows/ci.yml` Windows `fetch_pawnio_modules.ps1` step before `cargo test` / `cargo build` so `include_bytes!` resolves in CI.
