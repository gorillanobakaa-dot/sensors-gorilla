# PawnIO Modules (Optional Silicon & ThinkPad EC Sensors)

`sensors-gorilla` and `sensors-gorilla-l15` work on Windows with zero extra drivers.

When the signed [PawnIO](https://pawnio.eu) driver (`namazso.PawnIO`) is installed and the dashboard is run elevated (Administrator), it can optionally load the unmodified signed modules from [namazso/PawnIO.Modules](https://github.com/namazso/PawnIO.Modules) release `0.2.12` (LGPL-2.1, see `COPYING`):

- `IntelMSR.bin` — read-only Intel MSRs (CPU Package Temp, per-core P/E-core Temps, RAPL Package/Cores/Iris Xe Watts, throttle reasons)
- `IntelPCHThermal.bin` — read-only Intel PCH chipset temperature
- `AMDFamily17.bin` — read-only AMD Ryzen CPU Tctl/Tdie & power (Generic Windows on AMD)
- `LpcThinkpadEC.bin` — read-only ThinkPad sideband EC (`0x1600`/`0x1604`) for Fan RPM and EC thermals (ThinkPad L15 build only; never writes to the EC)

Every module's SHA-256 is verified against `modules.sha256` before loading. Any mismatch fails closed and refuses to load the module.

Fetch & verify locally with:

```powershell
.\scripts\fetch_pawnio_modules.ps1
```
