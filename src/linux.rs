//! Linux hardware sensor dashboard (Sony VAIO SVE14 + generic Linux hwmon/NVMe/sysfs).
//! Compiled native binary — zero Python interpreter, zero on-the-fly compilation.

#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::ansi;
use crate::posix;
use crate::ssd;

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn read_scaled(path: &Path, scale: f64) -> Option<f64> {
    let raw = read_trimmed(path)?;
    raw.parse::<f64>().ok().map(|v| v / scale)
}

pub fn get_cpu_temps() -> BTreeMap<String, f64> {
    let mut readings = BTreeMap::new();

    // Scan /sys/class/hwmon for coretemp, k10temp, zenpower
    let mut hwmon_dirs: Vec<PathBuf> = fs::read_dir("/sys/class/hwmon")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    hwmon_dirs.sort();

    for dir in &hwmon_dirs {
        let name = read_trimmed(&dir.join("name")).unwrap_or_default().to_lowercase();
        if matches!(name.as_str(), "coretemp" | "k10temp" | "zenpower") {
            let mut inputs: Vec<PathBuf> = fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("temp") && n.ends_with("_input"))
                        .unwrap_or(false)
                })
                .collect();
            inputs.sort();
            for inp in inputs {
                if let Some(temp) = read_scaled(&inp, 1000.0) {
                    let prefix = inp
                        .file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|n| n.split('_').next())
                        .unwrap_or("temp");
                    let raw_label = read_trimmed(&dir.join(format!("{prefix}_label")))
                        .unwrap_or_else(|| format!("Sensor {prefix}"));
                    let label = if raw_label == "Package id 0" {
                        "CPU/GPU Package Peak (Package id 0)".to_string()
                    } else if raw_label.starts_with("Core") {
                        format!("{raw_label} (CPU Core)")
                    } else {
                        raw_label
                    };
                    readings.insert(label, temp);
                }
            }
            if !readings.is_empty() {
                return readings;
            }
        }
    }

    // Fallback: /sys/class/thermal/thermal_zone*
    let mut zones: Vec<PathBuf> = fs::read_dir("/sys/class/thermal")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("thermal_zone"))
                .unwrap_or(false)
        })
        .collect();
    zones.sort();
    for z in zones {
        if let Some(temp) = read_scaled(&z.join("temp"), 1000.0) {
            let ztype = read_trimmed(&z.join("type")).unwrap_or_else(|| {
                z.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("zone")
                    .to_string()
            });
            readings.insert(format!("Thermal Zone ({ztype})"), temp);
        }
    }
    readings
}

fn get_motherboard_temp() -> Option<f64> {
    let mut dirs: Vec<PathBuf> = fs::read_dir("/sys/class/hwmon")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for d in &dirs {
        let name = read_trimmed(&d.join("name")).unwrap_or_default().to_lowercase();
        if name == "acpitz" || name == "pch_cannonlake" || name.contains("pch") {
            if let Some(t) = read_scaled(&d.join("temp1_input"), 1000.0) {
                return Some(t);
            }
        }
    }
    read_scaled(Path::new("/sys/class/hwmon/hwmon1/temp1_input"), 1000.0)
}

fn get_amd_gpu_temp() -> Option<f64> {
    for entry in fs::read_dir("/sys/class/hwmon").into_iter().flatten().flatten() {
        let dir = entry.path();
        let name = read_trimmed(&dir.join("name")).unwrap_or_default().to_lowercase();
        if name.contains("radeon") || name.contains("amdgpu") {
            if let Some(t) = read_scaled(&dir.join("temp1_input"), 1000.0) {
                return Some(t);
            }
        }
    }
    None
}

fn get_hwmon_fans() -> Vec<(String, u64)> {
    let mut fans = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir("/sys/class/hwmon")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for d in dirs {
        let chip = read_trimmed(&d.join("name")).unwrap_or_else(|| "hwmon".to_string());
        for entry in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = entry.path();
            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if fname.starts_with("fan") && fname.ends_with("_input") {
                if let Some(rpm_s) = read_trimmed(&p) {
                    if let Ok(rpm) = rpm_s.parse::<u64>() {
                        let prefix = fname.split('_').next().unwrap_or("fan");
                        let lbl = read_trimmed(&d.join(format!("{prefix}_label")))
                            .unwrap_or_else(|| format!("{chip}/{prefix}"));
                        fans.push((lbl, rpm));
                    }
                }
            }
        }
    }
    fans
}

#[derive(Default)]
struct SsdMetrics {
    model: String,
    capacity_bytes: u64,
    temp: Option<f64>,
    life_pct: Option<u8>,
    pct_used: Option<u8>,
    hours: Option<u64>,
    written_gb: Option<f64>,
}

fn find_primary_block_dev() -> String {
    if Path::new("/sys/block/nvme0n1").exists() && !Path::new("/sys/block/sda").exists() {
        return "/dev/nvme0n1".to_string();
    }
    if Path::new("/sys/block/sda").exists() {
        return "/dev/sda".to_string();
    }
    for entry in fs::read_dir("/sys/block").into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("nvme") || name.starts_with("sd") {
            return format!("/dev/{name}");
        }
    }
    "/dev/sda".to_string()
}

fn get_ssd_metrics() -> SsdMetrics {
    let mut m = SsdMetrics::default();
    let dev = find_primary_block_dev();
    let base_name = Path::new(&dev).file_name().and_then(|n| n.to_str()).unwrap_or("sda");
    if let Some(sz_s) = read_trimmed(&PathBuf::from(format!("/sys/block/{base_name}/size"))) {
        if let Ok(sectors) = sz_s.parse::<u64>() {
            m.capacity_bytes = sectors * 512;
        }
    }
    if let Some(model) = read_trimmed(&PathBuf::from(format!("/sys/block/{base_name}/device/model"))) {
        m.model = model;
    }

    // Also try nvme hwmon temp without sudo if present
    for entry in fs::read_dir("/sys/class/hwmon").into_iter().flatten().flatten() {
        let d = entry.path();
        if read_trimmed(&d.join("name")).as_deref() == Some("nvme") {
            if let Some(t) = read_scaled(&d.join("temp1_input"), 1000.0) {
                m.temp = Some(t);
            }
        }
    }

    let out = Command::new("sudo").args(["-n", "smartctl", "-A", &dev]).output();
    let stdout = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => {
            if let Ok(o2) = Command::new("smartctl").args(["-A", &dev]).output() {
                if o2.status.success() {
                    String::from_utf8_lossy(&o2.stdout).to_string()
                } else {
                    return m;
                }
            } else {
                return m;
            }
        }
    };

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 10 {
            let attr_id = parts[0];
            let val = parts[3];
            let raw = parts[9];
            match attr_id {
                "194" => {
                    if let Ok(t) = raw.parse::<f64>() {
                        m.temp = Some(t);
                    }
                }
                "231" => {
                    if let Ok(v) = val.parse::<u8>() {
                        m.life_pct = Some(v);
                        m.pct_used = Some(100u8.saturating_sub(v));
                    }
                }
                "9" => {
                    if let Ok(h) = raw.parse::<u64>() {
                        m.hours = Some(h);
                    }
                }
                "241" => {
                    if let Ok(lbas) = raw.parse::<f64>() {
                        m.written_gb = Some(lbas * 32.0 / 1024.0);
                    }
                }
                _ => {}
            }
        } else if line.starts_with("Temperature:") {
            if let Some(num) = line.split_whitespace().nth(1).and_then(|s| s.parse::<f64>().ok()) {
                m.temp = Some(num);
            }
        } else if line.starts_with("Percentage Used:") {
            if let Some(pct) = line.split(':').nth(1).and_then(|s| s.trim().trim_end_matches('%').parse::<u8>().ok()) {
                m.pct_used = Some(pct);
                m.life_pct = Some(100u8.saturating_sub(pct));
            }
        } else if line.starts_with("Power On Hours:") {
            let digits: String = line.split(':').nth(1).unwrap_or("").chars().filter(|c| c.is_ascii_digit()).collect();
            if let Ok(h) = digits.parse::<u64>() {
                m.hours = Some(h);
            }
        } else if line.starts_with("Data Units Written:") {
            let digits: String = line.split(':').nth(1).unwrap_or("").split('[').next().unwrap_or("")
                .chars().filter(|c| c.is_ascii_digit()).collect();
            if let Ok(du) = digits.parse::<f64>() {
                m.written_gb = Some(du * 512_000.0 / 1_000_000_000.0);
            }
        }
    }
    m
}

struct RaplPower {
    package: f64,
    core: Option<f64>,
    uncore: Option<f64>,
}

fn get_rapl_power() -> Option<RaplPower> {
    let mut paths: BTreeMap<String, PathBuf> = BTreeMap::new();
    for entry in fs::read_dir("/sys/class/powercap").into_iter().flatten().flatten() {
        let p = entry.path();
        let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !fname.starts_with("intel-rapl:") {
            continue;
        }
        if let Some(name) = read_trimmed(&p.join("name")) {
            let key = match name.as_str() {
                "package-0" => Some("package"),
                "core" => Some("core"),
                "uncore" => Some("uncore"),
                _ => None,
            };
            if let Some(k) = key {
                paths.insert(k.to_string(), p.join("energy_uj"));
            }
        }
    }
    if !paths.contains_key("package") {
        return None;
    }

    let keys: Vec<String> = paths.keys().cloned().collect();
    let read_all = || -> Option<(Vec<u64>, Instant)> {
        let mut vals = Vec::with_capacity(keys.len());
        let direct_ok = keys.iter().all(|k| {
            read_trimmed(&paths[k])
                .and_then(|s| s.parse::<u64>().ok())
                .map(|v| {
                    vals.push(v);
                    true
                })
                .unwrap_or(false)
        });
        if direct_ok {
            return Some((vals, Instant::now()));
        }
        vals.clear();
        let mut args = vec!["-n", "cat"];
        let str_paths: Vec<String> = keys.iter().map(|k| paths[k].to_string_lossy().to_string()).collect();
        for sp in &str_paths {
            args.push(sp.as_str());
        }
        let out = Command::new("sudo").args(&args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let parsed: Vec<u64> = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .filter_map(|w| w.parse::<u64>().ok())
            .collect();
        if parsed.len() == keys.len() {
            Some((parsed, Instant::now()))
        } else {
            None
        }
    };

    let (e1, t1) = read_all()?;
    thread::sleep(Duration::from_millis(250));
    let (e2, t2) = read_all()?;
    let dt = t2.duration_since(t1).as_secs_f64();
    if dt <= 0.0 {
        return None;
    }

    let mut map = BTreeMap::new();
    for ((k, a), b) in keys.iter().zip(e1.iter()).zip(e2.iter()) {
        if b >= a {
            map.insert(k.as_str(), (*b - *a) as f64 / dt / 1_000_000.0);
        }
    }
    let pkg = *map.get("package")?;
    Some(RaplPower {
        package: pkg,
        core: map.get("core").copied(),
        uncore: map.get("uncore").copied(),
    })
}

fn get_frequencies() -> (Option<f64>, Option<u64>, Option<u64>) {
    let mut max_khz: Option<u64> = None;
    for entry in fs::read_dir("/sys/devices/system/cpu").into_iter().flatten().flatten() {
        let p = entry.path().join("cpufreq/scaling_cur_freq");
        if let Some(v) = read_trimmed(&p).and_then(|s| s.parse::<u64>().ok()) {
            max_khz = Some(max_khz.map_or(v, |m| m.max(v)));
        }
    }
    let cpu_ghz = max_khz.map(|k| k as f64 / 1_000_000.0);

    let mut gpu_cur = None;
    let mut gpu_max = None;
    for entry in fs::read_dir("/sys/class/drm").into_iter().flatten().flatten() {
        let p = entry.path();
        if let Some(cur) = read_trimmed(&p.join("gt_cur_freq_mhz")).and_then(|s| s.parse::<u64>().ok()) {
            gpu_cur = Some(cur);
            gpu_max = read_trimmed(&p.join("gt_max_freq_mhz")).and_then(|s| s.parse::<u64>().ok());
            break;
        }
    }
    (cpu_ghz, gpu_cur, gpu_max)
}

fn get_throttle_stats() -> Option<(u64, u64, u64)> {
    let base = Path::new("/sys/devices/system/cpu/cpu0/thermal_throttle");
    let pkg = read_trimmed(&base.join("package_throttle_count"))?.parse::<u64>().ok()?;
    let pkg_ms = read_trimmed(&base.join("package_throttle_total_time_ms"))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let mut cores = 0u64;
    for entry in fs::read_dir("/sys/devices/system/cpu").into_iter().flatten().flatten() {
        let p = entry.path().join("thermal_throttle/core_throttle_count");
        if let Some(v) = read_trimmed(&p).and_then(|s| s.parse::<u64>().ok()) {
            cores += v;
        }
    }
    Some((pkg, pkg_ms, cores))
}

fn get_memory() -> Option<(f64, f64, f64, f64)> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    let mut map = BTreeMap::new();
    for line in content.lines() {
        if let Some((k, rest)) = line.split_once(':') {
            if let Some(kb) = rest.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                map.insert(k.trim().to_string(), kb / 1_048_576.0);
            }
        }
    }
    let total = *map.get("MemTotal")?;
    let avail = *map.get("MemAvailable").unwrap_or(&0.0);
    let swap_total = *map.get("SwapTotal").unwrap_or(&0.0);
    let swap_free = *map.get("SwapFree").unwrap_or(&0.0);
    Some((total, avail, swap_total, (swap_total - swap_free).max(0.0)))
}

fn get_kernel_cooling() -> (Vec<String>, String) {
    let mut busy = Vec::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir("/sys/class/thermal")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("cooling_device"))
                .unwrap_or(false)
        })
        .collect();
    dirs.sort();
    for d in dirs {
        let t = read_trimmed(&d.join("type")).unwrap_or_else(|| "N/A".to_string());
        let cur = read_trimmed(&d.join("cur_state")).unwrap_or_else(|| "N/A".to_string());
        let mx = read_trimmed(&d.join("max_state")).unwrap_or_else(|| "N/A".to_string());
        *kinds.entry(t.clone()).or_insert(0) += 1;
        if cur != "N/A" && cur != "0" {
            busy.push(format!("{t} {cur}/{mx}"));
        }
    }
    let summary = kinds
        .into_iter()
        .map(|(t, n)| if n > 1 { format!("{t} x{n}") } else { t })
        .collect::<Vec<_>>()
        .join(", ");
    (busy, summary)
}

fn onoff(v: Option<&str>) -> String {
    match v {
        Some("0") => "off".to_string(),
        Some("1") => "on".to_string(),
        Some("-1") => "not available".to_string(),
        Some(s) => s.to_string(),
        None => "N/A".to_string(),
    }
}

pub fn build_dashboard() -> String {
    let cap = ansi::detect_color_cap();
    let cpu_temps = get_cpu_temps();
    let mboard_temp = get_motherboard_temp();
    let sony_dir = Path::new("/sys/devices/platform/sony-laptop");
    let has_sony = sony_dir.is_dir();

    let fan_speed = read_trimmed(&sony_dir.join("fanspeed")).map(|s| format!("{s}%"));
    let thermal_profile = read_trimmed(&sony_dir.join("thermal_control")).unwrap_or_else(|| "N/A".to_string());
    let hwmon_fans = get_hwmon_fans();

    let ac_online = read_trimmed(Path::new("/sys/class/power_supply/ADP1/online"))
        .or_else(|| read_trimmed(Path::new("/sys/class/power_supply/AC/online")));
    let power_source = if ac_online.as_deref() == Some("1") { "AC Adapter" } else { "Battery" };
    let bat_charge = read_trimmed(Path::new("/sys/class/power_supply/BAT0/capacity")).unwrap_or_else(|| "N/A".to_string());
    let bat_status = read_trimmed(Path::new("/sys/class/power_supply/BAT0/status")).unwrap_or_else(|| "N/A".to_string());
    let bat_volts = read_scaled(Path::new("/sys/class/hwmon/hwmon2/in0_input"), 1000.0)
        .or_else(|| read_scaled(Path::new("/sys/class/power_supply/BAT0/voltage_now"), 1_000_000.0))
        .map(|v| format!("{v:.2} V"))
        .unwrap_or_else(|| "N/A".to_string());
    let bat_power = read_scaled(Path::new("/sys/class/hwmon/hwmon2/power1_input"), 1_000_000.0)
        .or_else(|| read_scaled(Path::new("/sys/class/power_supply/BAT0/power_now"), 1_000_000.0))
        .map(|w| format!("{w:.2} W"))
        .unwrap_or_else(|| "N/A".to_string());

    let ef = read_trimmed(Path::new("/sys/class/power_supply/BAT0/energy_full")).and_then(|s| s.parse::<f64>().ok());
    let efd = read_trimmed(Path::new("/sys/class/power_supply/BAT0/energy_full_design")).and_then(|s| s.parse::<f64>().ok());
    let bat_health = match (ef, efd) {
        (Some(f), Some(d)) if d > 0.0 => format!("{:.1}%", f / d * 100.0),
        _ => "N/A".to_string(),
    };
    let bat_cycles = read_trimmed(Path::new("/sys/class/power_supply/BAT0/cycle_count")).unwrap_or_else(|| "N/A".to_string());
    let care_limit = match read_trimmed(&sony_dir.join("battery_care_limiter")).as_deref() {
        Some("0") => "Off (full charge)".to_string(),
        Some(v) => format!("{v}% cap"),
        None => "N/A".to_string(),
    };

    let mut left = vec![ansi::paint("[SYSTEM TEMPERATURES]", ansi::COL_HEADER, true, cap)];
    let pkg_temp = cpu_temps
        .iter()
        .find(|(k, _)| k.contains("Package"))
        .map(|(_, v)| *v);
    left.push(format!(
        "  {:<47} : {}",
        "CPU/GPU Package Peak",
        ansi::fmt_temp(pkg_temp, 70.0, 80.0, cap)
    ));
    for (k, v) in &cpu_temps {
        if k.contains("Core") || !k.contains("Package") {
            left.push(format!(
                "  {} {:<44} : {}",
                ansi::paint("├─", ansi::COL_TREE, false, cap),
                k,
                ansi::fmt_temp(Some(*v), 70.0, 80.0, cap)
            ));
        }
    }
    left.push(format!(
        "  {} {:<44} : {}",
        ansi::paint("├─", ansi::COL_TREE, false, cap),
        "Motherboard / Chipset (ACPI Zone)",
        ansi::fmt_temp(mboard_temp, 70.0, 80.0, cap)
    ));
    let amd_temp = get_amd_gpu_temp();
    let gpu_line = match amd_temp {
        Some(t) => ansi::fmt_temp(Some(t), 70.0, 80.0, cap),
        None => ansi::paint("OFF / Not present", ansi::COL_MUTED, false, cap),
    };
    left.push(format!(
        "  {} {:<44} : {}",
        ansi::paint("├─", ansi::COL_TREE, false, cap),
        "Discrete GPU (amdgpu / radeon)",
        gpu_line
    ));

    let ssd_m = get_ssd_metrics();
    left.push(format!(
        "  {} {:<44} : {}",
        ansi::paint("└─", ansi::COL_TREE, false, cap),
        if ssd_m.model.is_empty() { "System SSD" } else { ssd_m.model.as_str() },
        ansi::fmt_temp(ssd_m.temp, 50.0, 60.0, cap)
    ));

    left.push(String::new());
    left.push(ansi::paint("[SYSTEM COOLING]", ansi::COL_HEADER, true, cap));
    if let Some(fs) = fan_speed {
        left.push(format!("  System Fan Speed          : {}", ansi::paint(&fs, ansi::COL_FREQ, false, cap)));
        left.push(format!("  Active Thermal Profile    : {}", ansi::paint(&thermal_profile, ansi::COL_FREQ, false, cap)));
    } else if !hwmon_fans.is_empty() {
        for (name, rpm) in &hwmon_fans {
            left.push(format!(
                "  {:<26}: {}",
                name,
                ansi::paint(&format!("{rpm} rpm"), ansi::COL_FREQ, false, cap)
            ));
        }
    } else {
        left.push("  System Fan Speed          : N/A".to_string());
    }

    let daemon_active = Command::new("systemctl")
        .args(["is-active", "thermal-profile-daemon"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false);
    left.push(format!(
        "  Fan Profile Daemon        : {}",
        if daemon_active {
            format!("{} (auto-stepping)", ansi::paint("active", ansi::COL_COOL, false, cap))
        } else {
            ansi::paint("not running", ansi::COL_MUTED, false, cap)
        }
    ));

    if let Some(rapl) = get_rapl_power() {
        let mut parts = Vec::new();
        if let Some(c) = rapl.core {
            parts.push(format!("cores {c:.1}"));
        }
        if let Some(u) = rapl.uncore {
            parts.push(format!("gpu/uncore {u:.1}"));
        }
        let detail = if parts.is_empty() {
            String::new()
        } else {
            format!("  ({} W)", parts.join(" + "))
        };
        left.push(format!(
            "  CPU Package Power (RAPL)  : {}{}",
            ansi::paint(&format!("{:.1} W", rapl.package), ansi::COL_POWER, false, cap),
            detail
        ));
    } else {
        left.push("  CPU Package Power (RAPL)  : N/A".to_string());
    }

    let (cpu_ghz, gpu_cur, gpu_max) = get_frequencies();
    let cpu_s = cpu_ghz.map(|g| format!("{g:.2} GHz")).unwrap_or_else(|| "N/A".to_string());
    let gpu_s = match (gpu_cur, gpu_max) {
        (Some(c), Some(mx)) => format!("{c} MHz (max {mx})"),
        (Some(c), None) => format!("{c} MHz"),
        _ => "N/A".to_string(),
    };
    left.push(format!(
        "  CPU / GPU Frequency       : {}  |  {}",
        ansi::paint(&cpu_s, ansi::COL_FREQ, false, cap),
        ansi::paint(&gpu_s, ansi::COL_FREQ, false, cap)
    ));

    let throt_str = match get_throttle_stats() {
        None => "N/A".to_string(),
        Some((0, _, 0)) => ansi::paint("none since boot", ansi::COL_COOL, false, cap),
        Some((pkg, ms, cores)) => ansi::paint(
            &format!("{pkg} pkg / {cores} core events, {:.1}s throttled", ms as f64 / 1000.0),
            ansi::COL_HOT,
            false,
            cap,
        ),
    };
    left.push(format!("  Thermal Throttling        : {throt_str}"));

    left.push(String::new());
    left.push(ansi::paint("[POWER SUPPLY & BATTERY]", ansi::COL_HEADER, true, cap));
    left.push(format!("  Primary Power Source      : {}", ansi::paint(power_source, ansi::COL_WHITE, false, cap)));
    left.push(format!(
        "  Battery Charge            : {}  ({bat_status})",
        if bat_charge == "N/A" {
            "N/A".to_string()
        } else {
            ansi::paint(&format!("{bat_charge}%"), ansi::COL_COOL, false, cap)
        }
    ));
    left.push(format!("  {} Voltage Readout        : {bat_volts}", ansi::paint("├─", ansi::COL_TREE, false, cap)));
    left.push(format!("  {} Current Power Draw     : {bat_power}", ansi::paint("├─", ansi::COL_TREE, false, cap)));
    left.push(format!(
        "  {} Battery Wear (Health)  : {}  of original capacity",
        ansi::paint("├─", ansi::COL_TREE, false, cap),
        ansi::paint(&bat_health, ansi::COL_COOL, false, cap)
    ));
    left.push(format!("  {} Charge Cycle Count     : {bat_cycles}", ansi::paint("├─", ansi::COL_TREE, false, cap)));
    if has_sony {
        let care_health = read_trimmed(&sony_dir.join("battery_care_health"))
            .map(|s| format!("{s}%"))
            .unwrap_or_else(|| "N/A".to_string());
        left.push(format!("  {} Sony Care Limiter      : {care_limit}", ansi::paint("├─", ansi::COL_TREE, false, cap)));
        left.push(format!(
            "  {} Sony Care Health       : {care_health}  (firmware's own estimate)",
            ansi::paint("└─", ansi::COL_TREE, false, cap)
        ));
    }

    let profile = ssd::infer_profile(&ssd_m.model, ssd_m.capacity_bytes, ssd_m.written_gb, ssd_m.pct_used);
    let mut right = ssd::build_projection_lines(&profile, ssd_m.life_pct, ssd_m.written_gb, None, ssd_m.hours, &[], cap);

    right.push(String::new());
    right.push(ansi::paint("[LOAD, MEMORY & HARDWARE CONTROLS]", ansi::COL_HEADER, true, cap));
    if let Some((tot, avail, sw_tot, sw_used)) = get_memory() {
        let used = (tot - avail).max(0.0);
        let pct = if tot > 0.0 { 100.0 * used / tot } else { 0.0 };
        let col = ansi::pct_rgb(pct, true);
        let sw_col = if sw_used > 0.5 { ansi::COL_HOT } else { ansi::COL_COOL };
        right.push(format!(
            "  RAM / Swap In Use         : {}  |  swap {}",
            ansi::paint(&format!("{used:.1}/{tot:.1} GB ({pct:.0}%)"), col, false, cap),
            ansi::paint(&format!("{sw_used:.1}/{sw_tot:.1} GB"), sw_col, false, cap)
        ));
    }
    if let Some((l1, l5, l15)) = posix::loadavg() {
        let threads = thread::available_parallelism().map(|p| p.get()).unwrap_or(1);
        let no_turbo = read_trimmed(Path::new("/sys/devices/system/cpu/intel_pstate/no_turbo"));
        let turbo = match no_turbo.as_deref() {
            Some("0") => "on",
            Some(_) => "off",
            None => "N/A",
        };
        let gov = read_trimmed(Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"))
            .unwrap_or_else(|| "N/A".to_string());
        right.push(format!(
            "  Load / Turbo / Governor   : {l1:.1} {l5:.1} {l15:.1} ({threads} threads)  |  turbo {}  |  {}",
            ansi::paint(turbo, ansi::COL_FREQ, false, cap),
            ansi::paint(&gov, ansi::COL_FREQ, false, cap)
        ));
    }
    let (busy, kinds) = get_kernel_cooling();
    right.push(format!(
        "  Kernel Cooling            : {}",
        if busy.is_empty() {
            ansi::paint(&format!("none active  ({kinds})"), ansi::COL_COOL, false, cap)
        } else {
            ansi::paint(&busy.join(", "), ansi::COL_HOT, false, cap)
        }
    ));
    if has_sony {
        let ff = read_trimmed(&sony_dir.join("fan_forced"));
        let uc = read_trimmed(&sony_dir.join("usb_charge"));
        let lr = read_trimmed(&sony_dir.join("lid_resume_S5"));
        let tp = read_trimmed(&sony_dir.join("touchpad"));
        right.push(format!(
            "  Sony                      : fan forced {} · usb charge {} · lid resume {} · touchpad {}",
            onoff(ff.as_deref()),
            onoff(uc.as_deref()),
            onoff(lr.as_deref()),
            onoff(tp.as_deref())
        ));
    }

    let (cols, rows) = posix::terminal_size().unwrap_or((1_000_000, 1_000_000));
    let label = if has_sony {
        format!("SONY VAIO GORILLA SYSTEM DASHBOARD   v{}", ansi::VERSION)
    } else {
        format!("LINUX GORILLA SYSTEM DASHBOARD   v{}", ansi::VERSION)
    };
    ansi::assemble_dashboard(&left, &right, &label, cols, rows, "sensors-gorilla", false, cap)
}

pub fn build_json() -> String {
    let temps = get_cpu_temps();
    let mem = get_memory();
    let ssd_m = get_ssd_metrics();
    let (cpu_ghz, gpu_cur, gpu_max) = get_frequencies();
    let load = posix::loadavg();
    let temp_entries: Vec<String> = temps
        .iter()
        .map(|(k, v)| format!("\"{}\":{:.1}", k.replace('"', "\\\""), v))
        .collect();
    let (mem_tot, mem_avail, sw_tot, sw_used) = mem.unwrap_or((0.0, 0.0, 0.0, 0.0));
    let (l1, l5, l15) = load.unwrap_or((0.0, 0.0, 0.0));
    format!(
        "{{\"platform\":\"linux\",\"version\":\"{}\",\"cpu_temps_c\":{{{}}},\"cpu_ghz\":{},\"gpu_mhz\":{},\"gpu_max_mhz\":{},\"ssd_temp_c\":{},\"ssd_life_pct\":{},\"ram_used_gb\":{:.2},\"ram_total_gb\":{:.2},\"swap_used_gb\":{:.2},\"swap_total_gb\":{:.2},\"load\":[{:.2},{:.2},{:.2}]}}",
        ansi::VERSION,
        temp_entries.join(","),
        cpu_ghz.map(|v| format!("{v:.2}")).unwrap_or_else(|| "null".to_string()),
        gpu_cur.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
        gpu_max.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
        ssd_m.temp.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        ssd_m.life_pct.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
        (mem_tot - mem_avail).max(0.0),
        mem_tot,
        sw_used,
        sw_tot,
        l1,
        l5,
        l15
    )
}
