//! Windows dashboard renderer for both Generic Windows and ThinkPad L15 Gen 3 builds.
//! Utilizes Windows Terminal's full 24-bit TrueColor (16.7 million colors) capability.

#![cfg(target_os = "windows")]

use crate::ansi::{self, ColorCap};
use crate::ssd;
use crate::win_sensors::{self, WindowsTelemetry};

fn fmt_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m {}s", secs % 60)
    }
}

fn boost_mode_label(v: Option<u32>) -> &'static str {
    match v {
        Some(0) => "Disabled",
        Some(1) => "Enabled",
        Some(2) => "Aggressive",
        Some(3) => "Efficient Enabled",
        Some(4) => "Efficient Aggressive",
        Some(5) => "Aggressive At Guaranteed",
        Some(6) => "Efficient Aggressive At Guaranteed",
        _ => "Auto",
    }
}

fn live_cpu_ghz(t: &WindowsTelemetry) -> (Option<f64>, Option<f64>, Option<f64>) {
    // Overall, P-core cluster, E-core cluster live GHz = MaxMhz * (% Processor Performance / 100)
    let mut all = Vec::new();
    let mut p_vals = Vec::new();
    let mut e_vals = Vec::new();

    for p in &t.ppi {
        let idx = p.Number as usize;
        let base_mhz = p.MaxMhz as f64;
        let perf_pct = t.pdh.per_cpu_perf_pct.get(&idx).copied().or(t.pdh.cpu_perf_pct).unwrap_or(100.0);
        let ghz = (base_mhz * perf_pct / 100.0) / 1000.0;
        if ghz > 0.1 {
            all.push(ghz);
            let is_p = t.topology.logical_map.get(idx).map(|(_, p)| *p).unwrap_or(true);
            if is_p {
                p_vals.push(ghz);
            } else {
                e_vals.push(ghz);
            }
        }
    }
    let avg = |v: &[f64]| -> Option<f64> {
        if v.is_empty() { None } else { Some(v.iter().sum::<f64>() / v.len() as f64) }
    };
    let max_all = all.iter().copied().reduce(f64::max);
    (max_all, avg(&p_vals), avg(&e_vals))
}

pub fn render(t: &WindowsTelemetry, is_l15: bool) -> String {
    let cap = ColorCap::TrueColor;
    let mut left: Vec<String> = Vec::new();

    // 1. [SYSTEM TEMPERATURES]
    left.push(ansi::paint("[SYSTEM TEMPERATURES]", ansi::COL_HEADER, true, cap));

    let pkg_temp = t
        .pawnio
        .package_temp_c
        .or_else(|| t.external.lhm_temps.get("CPU Package").copied())
        .or_else(|| t.external.hwinfo_temps.get("CPU Package").copied());

    if is_l15 {
        left.push(format!(
            "  {:<34} : {}",
            "CPU/Iris Xe Package (i7-1255U)",
            ansi::fmt_temp(pkg_temp, 75.0, 90.0, cap)
        ));
    } else {
        let cpu_lbl = if t.cpu_name.is_empty() {
            "CPU Package".to_string()
        } else {
            let short = t.cpu_name
                .replace("12th Gen Intel(R) Core(TM) ", "Intel ")
                .replace("13th Gen Intel(R) Core(TM) ", "Intel ")
                .replace("(R)", "")
                .replace("(TM)", "");
            format!("CPU Pkg ({})", short.trim().chars().take(24).collect::<String>())
        };
        left.push(format!(
            "  {:<34} : {}",
            cpu_lbl,
            ansi::fmt_temp(pkg_temp, 75.0, 90.0, cap)
        ));
    }

    let fmt_core_chip = |prefix: &str, cid: usize, temp: f64| -> String {
        let rgb = ansi::thermal_rgb(temp, 42.0, 75.0, 90.0);
        format!("{prefix}{cid} {}", ansi::paint(&format!("{temp:.0}°C"), rgb, true, cap))
    };

    if !t.pawnio.core_temps_c.is_empty() {
        if t.topology.e_cores > 0 {
            let p_list: Vec<String> = t
                .pawnio
                .core_temps_c
                .iter()
                .filter(|(_, is_p, _)| *is_p)
                .map(|(cid, _, temp)| fmt_core_chip("P", *cid, *temp))
                .collect();
            if !p_list.is_empty() {
                left.push(format!(
                    "  {} {:<31} : {}",
                    ansi::paint("├─", ansi::COL_TREE, false, cap),
                    format!("P-Cores ({}P)", p_list.len()),
                    p_list.join(" · ")
                ));
            }
            let e_all: Vec<(usize, f64)> = t
                .pawnio
                .core_temps_c
                .iter()
                .filter(|(_, is_p, _)| !*is_p)
                .map(|(cid, _, temp)| (*cid, *temp))
                .collect();
            for chunk in e_all.chunks(4) {
                let first = chunk.first().map(|(c, _)| *c).unwrap_or(0);
                let last = chunk.last().map(|(c, _)| *c).unwrap_or(0);
                let vals: Vec<String> = chunk
                    .iter()
                    .map(|(cid, temp)| fmt_core_chip("E", *cid, *temp))
                    .collect();
                left.push(format!(
                    "  {} {:<31} : {}",
                    ansi::paint("├─", ansi::COL_TREE, false, cap),
                    format!("E-Cores ({first}-{last})"),
                    vals.join(" · ")
                ));
            }
        } else {
            for chunk in t.pawnio.core_temps_c.chunks(4) {
                let first = chunk.first().map(|(c, _, _)| *c).unwrap_or(0);
                let last = chunk.last().map(|(c, _, _)| *c).unwrap_or(0);
                let vals: Vec<String> = chunk
                    .iter()
                    .map(|(cid, _, temp)| fmt_core_chip("C", *cid, *temp))
                    .collect();
                left.push(format!(
                    "  {} {:<31} : {}",
                    ansi::paint("├─", ansi::COL_TREE, false, cap),
                    format!("Cores ({first}-{last})"),
                    vals.join(" · ")
                ));
            }
        }
    } else if !t.external.lhm_temps.is_empty() {
        for (k, v) in t.external.lhm_temps.iter().take(6) {
            if k.contains("Core") || k.contains("CCD") || k.contains("Tdie") {
                left.push(format!(
                    "  {} {:<31} : {}",
                    ansi::paint("├─", ansi::COL_TREE, false, cap),
                    k.chars().take(31).collect::<String>(),
                    ansi::fmt_temp(Some(*v), 75.0, 90.0, cap)
                ));
            }
        }
    }

    if let Some(pch) = t.pawnio.pch_temp_c {
        left.push(format!(
            "  {} {:<31} : {}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            "Motherboard / PCH Chipset",
            ansi::fmt_temp(Some(pch), 70.0, 85.0, cap)
        ));
    }

    for tz in &t.pdh.thermal_zones {
        let zname = if tz.name.contains("THM0") {
            "Chassis / ACPI (\\_TZ.THM0)".to_string()
        } else {
            format!("ACPI Thermal ({})", tz.name.chars().take(16).collect::<String>())
        };
        left.push(format!(
            "  {} {:<31} : {}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            zname,
            ansi::fmt_temp(Some(tz.temp_c), 70.0, 85.0, cap)
        ));
    }

    if is_l15 && !t.pawnio.tp_ec_temps.is_empty() {
        let summary = t
            .pawnio
            .tp_ec_temps
            .iter()
            .map(|(k, v)| format!("{} {}°C", k.trim_start_matches("EC "), v))
            .collect::<Vec<_>>()
            .join(" · ");
        left.push(format!(
            "  {} {:<31} : {}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            "ThinkPad EC Board Sensors",
            ansi::paint(&summary, ansi::COL_SUBHEADER, false, cap)
        ));
    }

    if let Some(bt) = t.battery_temp_c {
        left.push(format!(
            "  {} {:<31} : {}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            "Battery Pack Temperature",
            ansi::fmt_temp(Some(bt), 45.0, 60.0, cap)
        ));
    }

    for (label, tc) in &t.wmi_probes {
        left.push(format!(
            "  {} {:<31} : {}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            label.chars().take(31).collect::<String>(),
            ansi::fmt_temp(Some(*tc), 70.0, 85.0, cap)
        ));
    }

    if let Some(nv) = &t.external.nvml {
        let gname = if nv.name.is_empty() { "NVIDIA GPU" } else { nv.name.as_str() };
        left.push(format!(
            "  {} {:<31} : {}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            gname.chars().take(31).collect::<String>(),
            ansi::fmt_temp(nv.temp_c, 75.0, 85.0, cap)
        ));
    } else if !t.gpu_name.is_empty() {
        let g_temp = pkg_temp; // Integrated Intel Iris Xe / UHD shares the CPU package die
        let note = if t.gpu_name.contains("Intel") { " (Shared Die)" } else { "" };
        left.push(format!(
            "  {} {:<31} : {}{}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            t.gpu_name.chars().take(31).collect::<String>(),
            ansi::fmt_temp(g_temp, 75.0, 90.0, cap),
            ansi::paint(note, ansi::COL_MUTED, false, cap)
        ));
    }

    if t.drives.is_empty() {
        left.push(format!(
            "  {} {:<31} : N/A",
            ansi::paint("└─", ansi::COL_TREE, false, cap),
            "Storage Drive"
        ));
    } else {
        for (i, d) in t.drives.iter().enumerate() {
            let branch = if i + 1 == t.drives.len() { "└─" } else { "├─" };
            let name = if d.model.is_empty() {
                format!("PhysicalDrive{} ({})", d.index, d.bus_type)
            } else {
                format!("{} ({})", d.model.chars().take(24).collect::<String>(), d.bus_type)
            };
            let extra = d
                .extra_temps_c
                .first()
                .map(|s| format!("  (Sensor 1: {s:.1}°C)"))
                .unwrap_or_default();
            left.push(format!(
                "  {} {:<31} : {}{}",
                ansi::paint(branch, ansi::COL_TREE, false, cap),
                name,
                ansi::fmt_temp(d.temp_c, 55.0, 70.0, cap),
                ansi::paint(&extra, ansi::COL_MUTED, false, cap)
            ));
        }
    }

    // 2. [SYSTEM COOLING & SILICON POWER]
    left.push(String::new());
    left.push(ansi::paint("[SYSTEM COOLING & SILICON POWER]", ansi::COL_HEADER, true, cap));

    if let Some(rpm) = t.pawnio.tp_fan_rpm {
        let mode = t.pawnio.tp_fan_mode.as_deref().unwrap_or("BIOS");
        left.push(format!(
            "  ThinkPad Fan Speed        : {}  ({})",
            ansi::paint(&format!("{rpm} rpm"), ansi::COL_FREQ, true, cap),
            mode
        ));
    } else if !t.external.lhm_fans.is_empty() {
        for (name, rpm) in t.external.lhm_fans.iter().take(3) {
            left.push(format!(
                "  {:<26}: {}",
                name,
                ansi::paint(&format!("{rpm:.0} rpm"), ansi::COL_FREQ, false, cap)
            ));
        }
    } else if let Some(nv_fan) = t.external.nvml.as_ref().and_then(|n| n.fan_pct) {
        left.push(format!(
            "  GPU Fan Speed             : {}",
            ansi::paint(&format!("{nv_fan}%"), ansi::COL_FREQ, false, cap)
        ));
    } else {
        let msg = if t.pawnio.status.is_empty() {
            "EC/WMI not exposed"
        } else {
            t.pawnio.status.as_str()
        };
        left.push(format!(
            "  System Fan Speed          : {}",
            ansi::paint(msg, ansi::COL_MUTED, false, cap)
        ));
    }

    let rapl_pkg = t.pawnio.rapl_pkg_w.or(t.pdh.energy_pkg_w);
    let rapl_cores = t.pawnio.rapl_cores_w.or(t.pdh.energy_pp0_w);
    let rapl_uncore = t.pawnio.rapl_uncore_w.or(t.pdh.energy_pp1_w);
    if let Some(pkg_w) = rapl_pkg {
        let mut parts = Vec::new();
        if let Some(c) = rapl_cores {
            parts.push(format!("cores {c:.1}"));
        }
        if let Some(u) = rapl_uncore {
            parts.push(format!("igpu/uncore {u:.1}"));
        }
        if let Some(d) = t.pdh.energy_dram_w {
            parts.push(format!("dram {d:.1}"));
        }
        let detail = if parts.is_empty() {
            String::new()
        } else {
            format!("  ({} W)", parts.join(" + "))
        };
        left.push(format!(
            "  CPU Package Power (RAPL)  : {}{}",
            ansi::paint(&format!("{pkg_w:.1} W"), ansi::COL_POWER, true, cap),
            detail
        ));
    } else if let Some(pw) = t.external.lhm_power.get("CPU Package").copied() {
        left.push(format!(
            "  CPU Package Power         : {}",
            ansi::paint(&format!("{pw:.1} W"), ansi::COL_POWER, false, cap)
        ));
    } else {
        let msg = if t.pawnio.status.is_empty() {
            "N/A"
        } else {
            t.pawnio.status.as_str()
        };
        left.push(format!(
            "  CPU Package Power (RAPL)  : {}",
            ansi::paint(msg, ansi::COL_MUTED, false, cap)
        ));
    }
    if let Some(sys_w) = t.pdh.system_power_w {
        left.push(format!(
            "  System Power Meter        : {}",
            ansi::paint(&format!("{sys_w:.1} W"), ansi::COL_POWER, false, cap)
        ));
    }

    let (max_ghz, p_ghz, e_ghz) = live_cpu_ghz(t);
    if is_l15 && t.topology.e_cores > 0 {
        let p_s = p_ghz.map(|g| format!("{g:.2} GHz")).unwrap_or_else(|| "N/A".to_string());
        let e_s = e_ghz.map(|g| format!("{g:.2} GHz")).unwrap_or_else(|| "N/A".to_string());
        left.push(format!(
            "  Hybrid CPU Clocks         : P-Core {}  |  E-Core {}",
            ansi::paint(&p_s, ansi::COL_FREQ, false, cap),
            ansi::paint(&e_s, ansi::COL_FREQ, false, cap)
        ));
    } else {
        let c_s = max_ghz.map(|g| format!("{g:.2} GHz")).unwrap_or_else(|| "N/A".to_string());
        let perf_s = t.pdh.cpu_perf_pct.map(|p| format!("{p:.0}% perf")).unwrap_or_default();
        left.push(format!(
            "  CPU Peak Frequency        : {}  {}",
            ansi::paint(&c_s, ansi::COL_FREQ, false, cap),
            ansi::paint(&perf_s, ansi::COL_MUTED, false, cap)
        ));
    }
    if let (Some(c1), Some(c2), Some(c3)) = (t.pdh.cpu_c1_pct, t.pdh.cpu_c2_pct, t.pdh.cpu_c3_pct) {
        left.push(format!(
            "  CPU C-State Residency     : C1 {:.1}%  ·  C2 {:.1}%  ·  C3 {:.1}%",
            c1, c2, c3
        ));
    }

    let throt_str = if t.pawnio.throttle_pkg_now || !t.pawnio.throttle_reasons.is_empty() {
        ansi::paint(&t.pawnio.throttle_reasons.join(", "), ansi::COL_HOT, true, cap)
    } else if let Some(tr) = t.pdh.thermal_zones.first().and_then(|z| z.throttle_reasons) {
        if tr != 0 {
            ansi::paint(&format!("ACPI throttle mask 0x{tr:X}"), ansi::COL_HOT, false, cap)
        } else if t.pdh.perf_limit_flags.unwrap_or(0) != 0 && t.pdh.perf_limit_pct.unwrap_or(100.0) < 99.5 {
            ansi::paint(
                &format!(
                    "PPM {:.0}% cap (flags 0x{:X})",
                    t.pdh.perf_limit_pct.unwrap_or(100.0),
                    t.pdh.perf_limit_flags.unwrap_or(0)
                ),
                ansi::COL_WARM,
                false,
                cap,
            )
        } else {
            ansi::paint("none active", ansi::COL_COOL, false, cap)
        }
    } else {
        ansi::paint("none active", ansi::COL_COOL, false, cap)
    };
    left.push(format!("  Thermal Throttling        : {throt_str}"));

    let max_cap = t.max_cpu_pct.map(|p| format!("{p}% cap")).unwrap_or_else(|| "100%".to_string());
    left.push(format!(
        "  Power Plan / Boost        : {}  |  {} ({max_cap})",
        ansi::paint(&t.power_plan, ansi::COL_FREQ, false, cap),
        boost_mode_label(t.boost_mode)
    ));

    // 3. [POWER SUPPLY & BATTERY]
    left.push(String::new());
    left.push(ansi::paint("[POWER SUPPLY & BATTERY]", ansi::COL_HEADER, true, cap));
    let b = &t.battery;
    left.push(format!(
        "  Primary Power Source      : {}{}",
        ansi::paint(if b.ac_online { "AC Adapter" } else { "Battery" }, ansi::COL_WHITE, true, cap),
        if b.battery_saver {
            ansi::paint("  (Energy Saver ON)", ansi::COL_WARM, false, cap)
        } else {
            String::new()
        }
    ));
    if b.present {
        let chg_s = match b.charge_pct {
            Some(p) => {
                let state = if b.charging {
                    "Charging"
                } else if b.discharging {
                    "Discharging"
                } else {
                    "Full / AC Hold"
                };
                format!(
                    "{} {}  ({})",
                    ansi::gradient_bar(p, 10, false, cap),
                    ansi::paint(&format!("{p:.0}%"), ansi::pct_rgb(p, false), true, cap),
                    state
                )
            }
            None => "N/A".to_string(),
        };
        left.push(format!("  Battery Charge            : {chg_s}"));
        let volt_s = b.voltage_v.map(|v| format!("{v:.2} V")).unwrap_or_else(|| "N/A".to_string());
        let rate_s = b.rate_w.map(|w| format!("{w:.2} W")).unwrap_or_else(|| "0.00 W".to_string());
        left.push(format!(
            "  {} Voltage / Power Draw    : {volt_s}  |  {rate_s}",
            ansi::paint("├─", ansi::COL_TREE, false, cap)
        ));
        let hlth_s = match (b.health_pct, b.full_mwh, b.design_mwh) {
            (Some(h), Some(f), Some(d)) => format!(
                "{}  ({:.2} / {:.2} Wh, {})",
                ansi::paint(&format!("{h:.1}%"), ansi::pct_rgb(h, false), true, cap),
                f as f64 / 1000.0,
                d as f64 / 1000.0,
                if b.chemistry.is_empty() { "Li" } else { b.chemistry.as_str() }
            ),
            _ => "N/A".to_string(),
        };
        left.push(format!(
            "  {} Battery Wear (Health)   : {hlth_s}",
            ansi::paint("├─", ansi::COL_TREE, false, cap)
        ));
        let cyc_s = b.cycles.map(|c| c.to_string()).unwrap_or_else(|| "N/A".to_string());
        left.push(format!(
            "  {} Charge Cycle Count      : {cyc_s}",
            ansi::paint("└─", ansi::COL_TREE, false, cap)
        ));
    } else {
        left.push("  Battery                   : Not present (AC desktop)".to_string());
    }

    // Right column: Storage health + lifetime projections + System Load/Memory/GPU/Architecture
    let primary_drv = t.drives.first();
    let mut extra_ssd_lines = Vec::new();
    if let Some(d) = primary_drv {
        let spare = d.avail_spare_pct.map(|s| format!("{s}%")).unwrap_or_else(|| "N/A".to_string());
        let cyc = d.power_cycles.map(|c| c.to_string()).unwrap_or_else(|| "N/A".to_string());
        let uns = d.unsafe_shutdowns.map(|u| u.to_string()).unwrap_or_else(|| "N/A".to_string());
        let err = d.media_errors.unwrap_or(0);
        extra_ssd_lines.push(format!(
            "  Spare / Cycles / MediaErr   : {spare} spare  |  {cyc} cycles  |  {uns} unsafe  |  {err} err"
        ));
    }
    extra_ssd_lines.push(format!(
        "  Live Disk I/O               : R {:.1} MB/s  |  W {:.1} MB/s  ({:.0}% busy)  |  C: {:.1} GB free",
        t.pdh.disk_read_mbps, t.pdh.disk_write_mbps, t.pdh.disk_time_pct, t.c_drive_free_gb
    ));

    let profile = match primary_drv {
        Some(d) => ssd::infer_profile(&d.model, d.capacity_bytes, d.written_gb, d.pct_used),
        None => ssd::infer_profile("System SSD", 1_000_000_000_000, None, None),
    };
    let mut right = ssd::build_projection_lines(
        &profile,
        primary_drv.and_then(|d| d.life_pct),
        primary_drv.and_then(|d| d.written_gb),
        primary_drv.and_then(|d| d.read_gb),
        primary_drv.and_then(|d| d.power_on_hours),
        &extra_ssd_lines,
        cap,
    );

    right.push(String::new());
    let sec_title = if is_l15 {
        "[HYBRID CPU, MEMORY, IRIS XE & THINKPAD L15]"
    } else {
        "[LOAD, MEMORY, GPU & SYSTEM]"
    };
    right.push(ansi::paint(sec_title, ansi::COL_HEADER, true, cap));

    let mem_pct = if t.mem_total_gb > 0.0 {
        100.0 * t.mem_used_gb / t.mem_total_gb
    } else {
        0.0
    };
    let sw_pct = if t.commit_limit_gb > 0.0 {
        100.0 * t.commit_used_gb / t.commit_limit_gb
    } else {
        0.0
    };
    right.push(format!(
        "  RAM / Commit In Use         : {} {}  |  commit {:.1}/{:.1} GB ({:.0}%)",
        ansi::gradient_bar(mem_pct, 8, true, cap),
        ansi::paint(&format!("{:.1}/{:.1} GB ({mem_pct:.0}%)", t.mem_used_gb, t.mem_total_gb), ansi::pct_rgb(mem_pct, true), false, cap),
        t.commit_used_gb,
        t.commit_limit_gb,
        sw_pct
    ));

    let cpu_u = t.pdh.cpu_total_pct.unwrap_or(0.0);
    let topo_desc = if t.topology.e_cores > 0 {
        format!("{}P+{}E / {}T", t.topology.p_cores, t.topology.e_cores, t.topology.logical_cpus)
    } else {
        format!("{}C / {}T", t.topology.physical_cores, t.topology.logical_cpus)
    };
    right.push(format!(
        "  CPU Load / Topology         : {} {}  ({topo_desc})  |  {} procs / {} threads",
        ansi::gradient_bar(cpu_u, 8, true, cap),
        ansi::paint(&format!("{cpu_u:.1}%"), ansi::pct_rgb(cpu_u, true), false, cap),
        t.proc_count,
        t.thread_count
    ));

    right.push(format!(
        "  GPU Memory & Engines        : Ded {:.0} MB  Shr {:.0} MB  |  3D {:.0}%  Dec {:.0}%  Enc {:.0}%",
        t.pdh.gpu_dedicated_mb, t.pdh.gpu_shared_mb, t.pdh.gpu_3d_pct, t.pdh.gpu_decode_pct, t.pdh.gpu_encode_pct
    ));
    if !t.ram_spec.is_empty() || !t.display_mode.is_empty() {
        let br_s = t.brightness_pct.map(|b| format!(" ({b}% bright)")).unwrap_or_default();
        right.push(format!(
            "  Hardware & Panel            : {}  |  {}{br_s}",
            if t.ram_spec.is_empty() { "RAM" } else { t.ram_spec.as_str() },
            if t.display_mode.is_empty() { "Display" } else { t.display_mode.as_str() }
        ));
    }
    if !t.pdh.net_active_iface.is_empty() {
        let short_nic = t
            .pdh
            .net_active_iface
            .replace("[R]", "")
            .replace("(R)", "")
            .replace("  ", " ");
        right.push(format!(
            "  Network Link & Throughput   : {} ({:.0} Mbps)  |  RX {:.1} TX {:.1} KB/s",
            short_nic.trim().chars().take(26).collect::<String>(),
            t.pdh.net_link_mbps,
            t.pdh.net_rx_kbps,
            t.pdh.net_tx_kbps
        ));
    }
    if !t.hardware_badges.is_empty() {
        let mid = (t.hardware_badges.len() + 1) / 2;
        let line1 = t.hardware_badges[..mid].join(" · ");
        let line2 = t.hardware_badges[mid..].join(" · ");
        if line2.is_empty() {
            right.push(format!(
                "  Platform Sensors & Devices  : {}",
                ansi::paint(&line1, ansi::COL_SUBHEADER, false, cap)
            ));
        } else {
            right.push(format!(
                "  Platform Sensors & Buses    : {}",
                ansi::paint(&line1, ansi::COL_SUBHEADER, false, cap)
            ));
            right.push(format!(
                "  Integrated I/O & Security   : {}",
                ansi::paint(&line2, ansi::COL_SUBHEADER, false, cap)
            ));
        }
    }

    if is_l15 {
        if let Some(odo) = &t.lenovo_odometer {
            right.push(format!(
                "  ThinkPad Odometer           : {} thermal events  |  {} shock  |  {}h CPU uptime  |  BIOS {}",
                odo.thermal_events.unwrap_or(0),
                odo.shock_events.unwrap_or(0),
                odo.cpu_uptime_hrs.unwrap_or(0),
                if t.bios_version.is_empty() { "N/A" } else { t.bios_version.trim() }
            ));
        }
    } else {
        right.push(format!(
            "  System / Uptime             : {} {}  |  up {}",
            t.sys_manufacturer,
            t.sys_model,
            fmt_uptime(t.uptime_secs)
        ));
    }

    if !t.pawnio.status.is_empty() && !t.pawnio.active {
        right.push(ansi::paint(
            &format!("  Silicon Probe               : {}", t.pawnio.status),
            ansi::COL_MUTED,
            false,
            cap,
        ));
    }

    let (cols, rows) = win_sensors::terminal_size().unwrap_or((1_000_000, 1_000_000));
    let label = if is_l15 {
        format!("THINKPAD L15 GEN 3 GORILLA SYSTEM DASHBOARD   v{}", ansi::VERSION)
    } else {
        format!("WINDOWS GORILLA SYSTEM DASHBOARD   v{}", ansi::VERSION)
    };
    let hint_cmd = if is_l15 { "sensors-gorilla-l15" } else { "sensors-gorilla" };
    let compact = rows < 500_000;
    ansi::assemble_dashboard(&left, &right, &label, cols, rows, hint_cmd, compact, cap)
}

pub fn render_json(t: &WindowsTelemetry, is_l15: bool) -> String {
    let (max_ghz, p_ghz, e_ghz) = live_cpu_ghz(t);
    let tz_json: Vec<String> = t
        .pdh
        .thermal_zones
        .iter()
        .map(|z| format!("{{\"name\":\"{}\",\"temp_c\":{:.2}}}", z.name.replace('\\', "\\\\").replace('"', "\\\""), z.temp_c))
        .collect();
    let drv_json: Vec<String> = t
        .drives
        .iter()
        .map(|d| {
            format!(
                "{{\"index\":{},\"model\":\"{}\",\"bus\":\"{}\",\"temp_c\":{},\"life_pct\":{},\"written_gb\":{},\"power_on_hours\":{}}}",
                d.index,
                d.model.replace('"', "\\\""),
                d.bus_type,
                d.temp_c.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
                d.life_pct.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
                d.written_gb.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
                d.power_on_hours.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            )
        })
        .collect();
    let badges_json: Vec<String> = t
        .hardware_badges
        .iter()
        .map(|b| format!("\"{}\"", b.replace('"', "\\\"")))
        .collect();
    let rapl_pkg_json = t.pawnio.rapl_pkg_w.or(t.pdh.energy_pkg_w);
    format!(
        "{{\"platform\":\"windows\",\"variant\":\"{}\",\"version\":\"{}\",\"system\":\"{} {}\",\"cpu\":\"{}\",\"p_cores\":{},\"e_cores\":{},\"threads\":{},\"cpu_max_ghz\":{},\"p_core_ghz\":{},\"e_core_ghz\":{},\"cpu_util_pct\":{},\"package_temp_c\":{},\"pch_temp_c\":{},\"rapl_pkg_w\":{},\"system_power_w\":{},\"fan_rpm\":{},\"thermal_zones\":[{}],\"drives\":[{}],\"hardware_badges\":[{}],\"battery\":{{\"present\":{},\"ac_online\":{},\"charge_pct\":{},\"health_pct\":{},\"cycles\":{},\"voltage_v\":{},\"rate_w\":{}}},\"memory\":{{\"used_gb\":{:.2},\"total_gb\":{:.2},\"commit_used_gb\":{:.2},\"commit_limit_gb\":{:.2}}}}}",
        if is_l15 { "thinkpad-l15-gen3" } else { "generic" },
        ansi::VERSION,
        t.sys_manufacturer.replace('"', "\\\""),
        t.sys_model.replace('"', "\\\""),
        t.cpu_name.replace('"', "\\\""),
        t.topology.p_cores,
        t.topology.e_cores,
        t.topology.logical_cpus,
        max_ghz.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        p_ghz.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        e_ghz.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        t.pdh.cpu_total_pct.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        t.pawnio.package_temp_c.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        t.pawnio.pch_temp_c.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        rapl_pkg_json.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        t.pdh.system_power_w.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        t.pawnio.tp_fan_rpm.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
        tz_json.join(","),
        drv_json.join(","),
        badges_json.join(","),
        t.battery.present,
        t.battery.ac_online,
        t.battery.charge_pct.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        t.battery.health_pct.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        t.battery.cycles.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
        t.battery.voltage_v.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        t.battery.rate_w.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        t.mem_used_gb,
        t.mem_total_gb,
        t.commit_used_gb,
        t.commit_limit_gb,
    )
}
