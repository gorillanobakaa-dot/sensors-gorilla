# v3.0.3, for developers

## Scope

`v3.0.2..HEAD`:

- Replaced `requireAdministrator` in `assets/icons/sensors-gorilla.manifest` with `asInvoker` (keeping `PerMonitorV2` DPI awareness), fixing `ERROR_ELEVATION_REQUIRED` (`0x800702e4`) when Windows Terminal spawns a console `.exe` via `CreateProcess`.
- Added native `is_elevated()` (`OpenProcessToken` + `GetTokenInformation(TokenElevation)`) and `relaunch_as_admin()` (`ShellExecuteW("runas", exe, "--elevated-child ...")`) in `src/win_ffi.rs` and `src/win_sensors.rs`, invoked at startup in `src/lib.rs` on interactive Windows TTY launches (`--no-elevate` / `--once` / `--json` / `--version` / `--help` bypass UAC).
- Updated Inno Setup `[Icons]` in `installer/sensors-gorilla-generic.iss` and `installer/sensors-gorilla-l15.iss` to point Desktop and Start Menu shortcuts directly at `{app}\{#AppExe}` rather than `wt.exe`.
