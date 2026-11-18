//! macOS (Intel & Apple Silicon) hardware sensor dashboard.
//! Reads SMC directly through macOS IOKit (zero external crates, zero Python, zero on-the-fly compilation).

#![cfg(target_os = "macos")]

use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::process::Command;
use std::thread;

use crate::ansi;
use crate::posix;

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct KeyInfo {
    size: u32,
    data_type: u32,
    attr: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SmcParam {
    key: u32,
    vers: [u8; 6],
    plimit: [u8; 16],
    info: KeyInfo,
    result: u8,
    status: u8,
    cmd: u8,
    data32: u32,
    bytes: [u8; 32],
}

impl Default for SmcParam {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    static mach_task_self_: u32;
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IOServiceOpen(service: u32, owning_task: u32, conn_type: u32, connect: *mut u32) -> i32;
    fn IOServiceClose(connect: u32) -> i32;
    fn IOConnectCallStructMethod(
        connection: u32,
        selector: u32,
        input_struct: *const c_void,
        input_struct_cnt: usize,
        output_struct: *mut c_void,
        output_struct_cnt: *mut usize,
    ) -> i32;
}

const SMC_TEMPS: [(&str, &str); 12] = [
    ("TC0P", "CPU proximity"),
    ("TC0D", "CPU die"),
    ("TC0E", "CPU die 2"),
    ("TC0F", "CPU die 3"),
    ("TC0H", "CPU heatsink"),
    ("TG0P", "GPU proximity"),
    ("TG0D", "GPU die"),
    ("TPCD", "Platform controller"),
    ("TM0P", "Memory"),
    ("Th0H", "Heat pipe"),
    ("Th1H", "Heat pipe 2"),
    ("TB0T", "Battery"),
];

#[derive(Clone, Debug)]
struct FanInfo {
    rpm: f64,
    min: f64,
    max: f64,
}

struct SmcReading {
    temps: BTreeMap<String, f64>,
    fans: Vec<FanInfo>,
}

/// Decode AppleSMC fixed-point types (`sp78`, `fpe2`, `ui8`).
pub fn decode_smc_value(type_tag: &str, bytes: &[u8]) -> Option<f64> {
    if type_tag == "sp78" && bytes.len() >= 2 {
        let raw = i16::from_be_bytes([bytes[0], bytes[1]]);
        return Some((raw as f64 / 256.0 * 10.0).round() / 10.0);
    }
    if type_tag == "fpe2" && bytes.len() >= 2 {
        let raw = u16::from_be_bytes([bytes[0], bytes[1]]);
        return Some((raw as f64 / 4.0).round());
    }
    if type_tag.starts_with("ui8") && !bytes.is_empty() {
        return Some(bytes[0] as f64);
    }
    None
}

fn smc_read() -> Option<SmcReading> {
    let svc_name = CString::new("AppleSMC").ok()?;
    let mut conn: u32 = 0;
    unsafe {
        let matching = IOServiceMatching(svc_name.as_ptr());
        if matching.is_null() {
            return None;
        }
        let svc = IOServiceGetMatchingService(0, matching);
        if svc == 0 {
            return None;
        }
        if IOServiceOpen(svc, mach_task_self_, 0, &mut conn) != 0 {
            return None;
        }
    }

    let call = |p: &SmcParam| -> Option<SmcParam> {
        let mut out = SmcParam::default();
        let mut out_sz = std::mem::size_of::<SmcParam>();
        let rc = unsafe {
            IOConnectCallStructMethod(
                conn,
                2,
                p as *const SmcParam as *const c_void,
                std::mem::size_of::<SmcParam>(),
                &mut out as *mut SmcParam as *mut c_void,
                &mut out_sz,
            )
        };
        if rc == 0 && out.result == 0 {
            Some(out)
        } else {
            None
        }
    };

    let read_key = |key: &str| -> Option<f64> {
        let kb = key.as_bytes();
        if kb.len() != 4 {
            return None;
        }
        let mut p = SmcParam::default();
        p.key = u32::from_be_bytes([kb[0], kb[1], kb[2], kb[3]]);
        p.cmd = 9; // kSMCGetKeyInfo
        let info = call(&p)?;
        if info.info.size == 0 || info.info.size as usize > 32 {
            return None;
        }
        p.info.size = info.info.size;
        p.cmd = 5; // kSMCReadKey
        let out = call(&p)?;
        let tb = info.info.data_type.to_be_bytes();
        let tag = String::from_utf8_lossy(&tb);
        decode_smc_value(&tag, &out.bytes[..info.info.size as usize])
    };

    let mut temps = BTreeMap::new();
    for (k, _) in SMC_TEMPS {
        if let Some(v) = read_key(k) {
            if v > 0.0 && v < 130.0 {
                temps.insert(k.to_string(), v);
            }
        }
    }
    let fnum = read_key("FNum").unwrap_or(0.0) as usize;
    let mut fans = Vec::new();
    for i in 0..fnum.min(8) {
        if let Some(rpm) = read_key(&format!("F{i}Ac")) {
            let min = read_key(&format!("F{i}Mn")).unwrap_or(0.0);
            let max = read_key(&format!("F{i}Mx")).unwrap_or(0.0);
            fans.push(FanInfo { rpm, min, max });
        }
    }

    unsafe {
        IOServiceClose(conn);
    }
    Some(SmcReading { temps, fans })
}

fn run_cmd(prog: &str, args: &[&str]) -> String {
    Command::new(prog)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

struct BatteryInfo {
    charge: i64,
    full: i64,
    design: i64,
    cycles: i64,
    volts: f64,
    watts: f64,
    charging: bool,
    ac: bool,
    temp: Option<f64>,
}

fn mac_battery() -> Option<BatteryInfo> {
    let raw = run_cmd("ioreg", &["-rn", "AppleSmartBattery"]);
    let num = |k: &str| -> Option<i64> {
        let needle = format!("\"{k}\" = ");
        for line in raw.lines() {
            if let Some(idx) = line.find(&needle) {
                let rest = line[idx + needle.len()..].trim();
                if let Ok(v) = rest.split_whitespace().next()?.parse::<i128>() {
                    // Handle unsigned 64-bit wraparound for negative current
                    if v > i64::MAX as i128 {
                        return Some((v - (1i128 << 64)) as i64);
                    }
                    return Some(v as i64);
                }
            }
        }
        None
    };
    let design = num("DesignCapacity")?;
    let amps = num("InstantAmperage").or_else(|| num("Amperage")).unwrap_or(0);
    let volts = num("Voltage").unwrap_or(0) as f64 / 1000.0;
    let ac = raw.contains("\"ExternalConnected\" = Yes");
    let temp = num("Temperature").map(|t| t as f64 / 100.0).filter(|t| *t > 0.0);
    Some(BatteryInfo {
        charge: num("CurrentCapacity").unwrap_or(0),
        full: num("MaxCapacity").unwrap_or(0),
        design,
        cycles: num("CycleCount").unwrap_or(0),
        volts,
        watts: (volts * (amps as f64) / 1000.0).abs(),
        charging: amps > 0,
        ac,
        temp,
    })
}

fn mac_memory() -> (f64, f64, f64) {
    let vm = run_cmd("vm_stat", &[]);
    let mut ps = 4096.0f64;
    if let Some(idx) = vm.find("page size of ") {
        if let Some(v) = vm[idx + 13..].split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
            ps = v;
        }
    }
    let pages = |name: &str| -> f64 {
        for line in vm.lines() {
            if let Some(rest) = line.strip_prefix(name) {
                if let Some(num) = rest.trim().trim_start_matches(':').trim().trim_end_matches('.').parse::<f64>().ok() {
                    return num;
                }
            }
        }
        0.0
    };
    let total = run_cmd("sysctl", &["-n", "hw.memsize"])
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
        / 1_073_741_824.0;
    let used = (pages("Pages wired down") + pages("Pages active") + pages("Pages occupied by compressor")) * ps
        / 1_073_741_824.0;
    let sw = run_cmd("sysctl", &["vm.swapusage"]);
    let mut swap_used = 0.0f64;
    if let Some(idx) = sw.find("used = ") {
        let tok: String = sw[idx + 7..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if let Ok(m) = tok.parse::<f64>() {
            swap_used = m / 1024.0;
        }
    }
    (total, used, swap_used)
}

fn mac_browser_mb() -> Option<u64> {
    let out = run_cmd("ps", &["-A", "-o", "rss=,comm="]);
    let mut total_kb = 0u64;
    for line in out.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(rss), Some(comm)) = (parts.next(), parts.next()) {
            let base = comm.rsplit('/').next().unwrap_or(comm);
            if base == "firefox" || base == "plugin-container" {
                if let Ok(kb) = rss.parse::<u64>() {
                    total_kb += kb;
                }
            }
        }
    }
    if total_kb > 0 { Some(total_kb / 1024) } else { None }
}

fn mac_cpu_limit() -> Option<u32> {
    let out = run_cmd("pmset", &["-g", "therm"]);
    for line in out.lines() {
        if line.contains("CPU_Speed_Limit") {
            if let Some(v) = line.split('=').nth(1).and_then(|s| s.trim().parse::<u32>().ok()) {
                return Some(v);
            }
        }
    }
    None
}

fn mac_disk_free_gb() -> f64 {
    let out = run_cmd("df", &["-k", "/"]);
    if let Some(line) = out.lines().nth(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 4 {
            if let Ok(kb) = cols[3].parse::<f64>() {
                return kb / 1_048_576.0;
            }
        }
    }
    0.0
}

pub fn build_dashboard() -> String {
    let cap = ansi::detect_color_cap();
    let model = run_cmd("sysctl", &["-n", "hw.model"]).trim().to_string();
    let smc = smc_read();
    let (temps, fans) = match smc {
        Some(s) => (s.temps, s.fans),
        None => (BTreeMap::new(), Vec::new()),
    };

    let mut left = vec![ansi::paint("[TEMPERATURES]", ansi::COL_HEADER, true, cap)];
    if temps.is_empty() {
        left.push(format!("  {}", ansi::paint("SMC not readable (not an Intel Mac?)", ansi::COL_HOT, false, cap)));
    }
    if let Some(v) = temps.get("TC0P") {
        left.push(format!("  {:<22}: {}", "CPU proximity", ansi::fmt_temp(Some(*v), 70.0, 85.0, cap)));
    }
    let die: Vec<f64> = ["TC0D", "TC0E", "TC0F"]
        .iter()
        .filter_map(|k| temps.get(*k).copied())
        .collect();
    if let Some(max_die) = die.into_iter().reduce(f64::max) {
        left.push(format!("  {:<22}: {}", "CPU die (hottest)", ansi::fmt_temp(Some(max_die), 80.0, 95.0, cap)));
    }
    let name_map: BTreeMap<&str, &str> = SMC_TEMPS.iter().copied().collect();
    for k in ["TG0P", "TG0D", "TPCD", "TM0P", "Th0H", "Th1H", "TC0H"] {
        if let Some(v) = temps.get(k) {
            left.push(format!(
                "  {:<22}: {}",
                name_map.get(k).copied().unwrap_or(k),
                ansi::fmt_temp(Some(*v), 70.0, 85.0, cap)
            ));
        }
    }

    left.push(String::new());
    left.push(ansi::paint("[COOLING]", ansi::COL_HEADER, true, cap));
    if fans.is_empty() {
        left.push(format!("  {:<22}: N/A", "Fan"));
    } else {
        for (i, f) in fans.iter().enumerate() {
            let span = f.max - f.min;
            let pct = if span > 0.0 {
                format!(" ({:.0}% of range)", 100.0 * (f.rpm - f.min) / span)
            } else {
                String::new()
            };
            let lbl = if fans.len() > 1 { format!("Fan {}", i + 1) } else { "Fan".to_string() };
            left.push(format!(
                "  {:<22}: {}{}  min {:.0}, max {:.0}",
                lbl,
                ansi::paint(&format!("{:.0} rpm", f.rpm), ansi::COL_FREQ, false, cap),
                pct,
                f.min,
                f.max
            ));
        }
    }
    let mfc = !run_cmd("pgrep", &["-x", "Macs Fan Control"]).trim().is_empty();
    let fan_ctl = if mfc {
        ansi::paint("Macs Fan Control", ansi::COL_WARM, false, cap)
    } else {
        ansi::paint("macOS (automatic)", ansi::COL_COOL, false, cap)
    };
    left.push(format!("  {:<22}: {fan_ctl}", "Fan controlled by"));

    let lim = mac_cpu_limit();
    let lim_s = match lim {
        None => "N/A".to_string(),
        Some(l) if l >= 100 => ansi::paint("none (100%)", ansi::COL_COOL, false, cap),
        Some(l) => ansi::paint(&format!("{l}% (macOS is cooling the CPU)"), ansi::COL_HOT, false, cap),
    };
    left.push(format!("  {:<22}: {lim_s}", "CPU speed limit"));

    let mut right = vec![ansi::paint("[POWER & BATTERY]", ansi::COL_HEADER, true, cap)];
    if let Some(b) = mac_battery() {
        right.push(format!(
            "  {:<22}: {}",
            "Power source",
            ansi::paint(if b.ac { "AC adapter" } else { "Battery" }, ansi::COL_WHITE, false, cap)
        ));
        if b.full > 0 {
            let chg_pct = 100.0 * b.charge as f64 / b.full as f64;
            right.push(format!(
                "  {:<22}: {}  ({})",
                "Charge",
                ansi::paint(&format!("{chg_pct:.0}%"), ansi::COL_COOL, false, cap),
                if b.charging { "charging" } else { "not charging" }
            ));
        }
        if b.design > 0 {
            let hlth = 100.0 * b.full as f64 / b.design as f64;
            right.push(format!(
                "  {:<22}: {}  ({} of {} mAh)",
                "Battery health",
                ansi::paint(&format!("{hlth:.1}%"), ansi::pct_rgb(hlth, false), false, cap),
                b.full,
                b.design
            ));
        }
        right.push(format!("  {:<22}: {}", "Charge cycles", b.cycles));
        right.push(format!("  {:<22}: {:.2} V / {:.1} W", "Voltage / draw", b.volts, b.watts));
        if let Some(bt) = b.temp {
            right.push(format!("  {:<22}: {}", "Battery temperature", ansi::fmt_temp(Some(bt), 40.0, 45.0, cap)));
        }
    } else {
        right.push("  No battery (desktop Mac)".to_string());
    }

    right.push(String::new());
    right.push(ansi::paint("[LOAD & MEMORY]", ansi::COL_HEADER, true, cap));
    let (mem_tot, mem_used, sw_used) = mac_memory();
    let mem_pct = if mem_tot > 0.0 { 100.0 * mem_used / mem_tot } else { 0.0 };
    right.push(format!(
        "  {:<22}: {}  swap {sw_used:.1} GB",
        "RAM in use",
        ansi::paint(&format!("{mem_used:.1}/{mem_tot:.1} GB ({mem_pct:.0}%)"), ansi::pct_rgb(mem_pct, true), false, cap)
    ));
    let bm_s = match mac_browser_mb() {
        None => "not running".to_string(),
        Some(mb) => {
            let col = if mb <= 500 { ansi::COL_COOL } else { ansi::COL_WARM };
            format!("{}  (budget 500 MB)", ansi::paint(&format!("{mb} MB"), col, false, cap))
        }
    };
    right.push(format!("  {:<22}: {bm_s}", "Gorilla browser"));
    let (l1, l5, l15) = posix::loadavg().unwrap_or((0.0, 0.0, 0.0));
    let threads = thread::available_parallelism().map(|p| p.get()).unwrap_or(1);
    right.push(format!("  {:<22}: {l1:.1} {l5:.1} {l15:.1}  ({threads} threads)", "Load (1/5/15 min)"));
    right.push(format!("  {:<22}: {:.1} GB", "Disk free", mac_disk_free_gb()));

    let (cols, rows) = posix::terminal_size().unwrap_or((1_000_000, 1_000_000));
    let mname = if model.is_empty() { "MAC".to_string() } else { model.to_uppercase() };
    let label = format!("{mname} GORILLA SYSTEM DASHBOARD   v{}", ansi::VERSION);
    ansi::assemble_dashboard(&left, &right, &label, cols, rows, "sensors-gorilla", true, cap)
}

pub fn build_json() -> String {
    let model = run_cmd("sysctl", &["-n", "hw.model"]).trim().to_string();
    let (temp_map, fan_vec, smc_ok) = match smc_read() {
        Some(s) => {
            let ok = !s.temps.is_empty();
            (s.temps, s.fans, ok)
        }
        None => (BTreeMap::new(), Vec::new(), false),
    };
    let cpu_c = temp_map.get("TC0P").copied();
    let cpu_die_c = ["TC0D", "TC0E", "TC0F"]
        .iter()
        .filter_map(|k| temp_map.get(*k).copied())
        .reduce(f64::max);
    let temp_json: Vec<String> = temp_map.iter().map(|(k, v)| format!("\"{k}\":{v:.1}")).collect();
    let fan_json: Vec<String> = fan_vec
        .iter()
        .map(|f| format!("{{\"rpm\":{:.0},\"min\":{:.0},\"max\":{:.0}}}", f.rpm, f.min, f.max))
        .collect();
    let (mem_tot, mem_used, sw_used) = mac_memory();
    let (l1, l5, l15) = posix::loadavg().unwrap_or((0.0, 0.0, 0.0));
    let threads = thread::available_parallelism().map(|p| p.get()).unwrap_or(1);
    let mfc = !run_cmd("pgrep", &["-x", "Macs Fan Control"]).trim().is_empty();
    let bat_json = match mac_battery() {
        Some(b) => format!(
            "{{\"charge\":{},\"full\":{},\"design\":{},\"cycles\":{},\"volts\":{:.2},\"watts\":{:.2},\"charging\":{},\"ac\":{}}}",
            b.charge, b.full, b.design, b.cycles, b.volts, b.watts, b.charging, b.ac
        ),
        None => "{}".to_string(),
    };
    format!(
        "{{\"model\":\"{}\",\"smc_ok\":{},\"temps\":{{{}}},\"cpu_c\":{},\"cpu_die_c\":{},\"fans\":[{}],\"fan_control\":\"{}\",\"cpu_limit\":{},\"battery\":{},\"memory\":{{\"total\":{:.2},\"used\":{:.2},\"swap_used\":{:.2}}},\"browser_mb\":{},\"load\":[{:.2},{:.2},{:.2}],\"threads\":{},\"disk_free_gb\":{:.2}}}",
        model.replace('"', "\\\""),
        smc_ok,
        temp_json.join(","),
        cpu_c.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        cpu_die_c.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".to_string()),
        fan_json.join(","),
        if mfc { "Macs Fan Control" } else { "macOS (automatic)" },
        mac_cpu_limit().map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
        bat_json,
        mem_tot,
        mem_used,
        sw_used,
        mac_browser_mb().map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
        l1,
        l5,
        l15,
        threads,
        mac_disk_free_gb()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smc_fixed_point_decoders() {
        // sp78: 47.75 C -> 0x2FC0
        assert_eq!(decode_smc_value("sp78", &[0x2F, 0xC0]), Some(47.8));
        // fpe2: 2026 rpm -> 8104 -> 0x1FA8
        assert_eq!(decode_smc_value("fpe2", &[0x1F, 0xA8]), Some(2026.0));
        assert_eq!(decode_smc_value("ui8 ", &[2]), Some(2.0));
    }
}
