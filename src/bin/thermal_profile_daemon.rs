//! Compiled native Sony VAIO SVE14 thermal-profile-daemon (Linux).
//! Zero Python interpreter, zero on-the-fly compilation.

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("thermal-profile-daemon is for Linux (sony-laptop sysfs).");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
fn main() {
    linux_daemon::run();
}

#[cfg(target_os = "linux")]
mod linux_daemon {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    const POLL_S: u64 = 5;
    const STEP_UP_S: u64 = 0;
    const STEP_DOWN_S: u64 = 180;
    const SILENT_HOLD_S: u64 = 300;
    const BALANCED_TEMP: f64 = 72.0;
    const PERF_TEMP: f64 = 82.0;
    const HYSTERESIS: f64 = 6.0;
    const FORCE_TEMP: f64 = 93.0;
    const FORCE_SUSTAIN_S: u64 = 15;
    const UNFORCE_TEMP: f64 = 85.0;

    static STOP: AtomicBool = AtomicBool::new(false);

    extern "C" fn on_sig(_sig: i32) {
        STOP.store(true, Ordering::SeqCst);
    }

    extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }

    fn read_trim(p: &Path) -> Option<String> {
        fs::read_to_string(p).ok().map(|s| s.trim().to_string())
    }

    fn package_temp_c() -> Option<f64> {
        for entry in fs::read_dir("/sys/devices/platform/coretemp.0/hwmon").into_iter().flatten().flatten() {
            if let Some(s) = read_trim(&entry.path().join("temp1_input")) {
                if let Ok(milli) = s.parse::<f64>() {
                    return Some(milli / 1000.0);
                }
            }
        }
        read_trim(Path::new("/sys/class/thermal/thermal_zone0/temp"))
            .and_then(|s| s.parse::<f64>().ok())
            .map(|m| m / 1000.0)
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().collect();
        let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("status");
        let sony = Path::new("/sys/devices/platform/sony-laptop");
        let tc = sony.join("thermal_control");
        let tp = sony.join("thermal_profiles");
        let ff = sony.join("fan_forced");

        match cmd {
            "-h" | "--help" | "help" => {
                println!(
                    "thermal-profile-daemon {}\nusage: thermal-profile-daemon [run|status|set PROFILE]\n\nAutomatic Sony VAIO fan profile stepper (silent -> balanced -> performance).",
                    env!("CARGO_PKG_VERSION")
                );
            }
            "-V" | "--version" => {
                println!("thermal-profile-daemon {}", env!("CARGO_PKG_VERSION"));
            }
            "status" => {
                let t = package_temp_c().map(|v| format!("{v:.1}°C")).unwrap_or_else(|| "N/A".to_string());
                let cur = read_trim(&tc).unwrap_or_else(|| "N/A".to_string());
                let forced = read_trim(&ff).unwrap_or_else(|| "N/A".to_string());
                println!("Package: {t}  |  Profile: {cur}  |  Fan forced: {forced}");
            }
            "set" => {
                let target = args.get(2).map(|s| s.as_str()).unwrap_or("balanced");
                if let Err(e) = fs::write(&tc, target) {
                    eprintln!("Failed to set profile '{target}': {e}");
                    std::process::exit(1);
                }
                println!("Set thermal_control -> {target}");
            }
            "run" => {
                let profiles = read_trim(&tp).unwrap_or_default();
                for req in ["silent", "balanced", "performance"] {
                    if !profiles.split_whitespace().any(|w| w == req) {
                        eprintln!("sony-laptop does not offer the '{req}' profile (found: '{profiles}')");
                        std::process::exit(1);
                    }
                }
                unsafe {
                    signal(2, on_sig as *const () as usize);
                    signal(15, on_sig as *const () as usize);
                }
                let mut current = read_trim(&tc).unwrap_or_else(|| "balanced".to_string());
                let mut cool_since: Option<Instant> = None;
                let mut hot_since: Option<Instant> = None;
                let mut forced = read_trim(&ff).as_deref() == Some("1");

                while !STOP.load(Ordering::SeqCst) {
                    if let Some(t) = package_temp_c() {
                        let desired = if t >= PERF_TEMP {
                            "performance"
                        } else if t >= BALANCED_TEMP || (current == "performance" && t >= PERF_TEMP - HYSTERESIS) {
                            "balanced"
                        } else if current == "balanced" && t >= BALANCED_TEMP - HYSTERESIS {
                            "balanced"
                        } else {
                            "silent"
                        };

                        let rank = |p: &str| match p {
                            "silent" => 0,
                            "balanced" => 1,
                            _ => 2,
                        };

                        if rank(desired) > rank(&current) {
                            cool_since = None;
                            let _ = STEP_UP_S;
                            let _ = fs::write(&tc, desired);
                            current = desired.to_string();
                        } else if rank(desired) < rank(&current) {
                            let need = if desired == "silent" { SILENT_HOLD_S } else { STEP_DOWN_S };
                            let start = cool_since.get_or_insert_with(Instant::now);
                            if start.elapsed().as_secs() >= need {
                                let _ = fs::write(&tc, desired);
                                current = desired.to_string();
                                cool_since = None;
                            }
                        } else {
                            cool_since = None;
                        }

                        if t >= FORCE_TEMP {
                            let hs = hot_since.get_or_insert_with(Instant::now);
                            if !forced && hs.elapsed().as_secs() >= FORCE_SUSTAIN_S {
                                let _ = fs::write(&ff, "1");
                                forced = true;
                            }
                        } else {
                            hot_since = None;
                            if forced && t < UNFORCE_TEMP {
                                let _ = fs::write(&ff, "0");
                                forced = false;
                            }
                        }
                    }
                    thread::sleep(Duration::from_secs(POLL_S));
                }
                let _ = fs::write(&ff, "0");
                let _ = fs::write(&tc, "balanced");
                let _: PathBuf = sony.to_path_buf();
            }
            other => {
                eprintln!("Unknown subcommand: {other}");
                std::process::exit(2);
            }
        }
    }
}
