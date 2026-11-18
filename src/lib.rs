pub mod ansi;
pub mod sha256;
pub mod ssd;

#[cfg(unix)]
pub mod posix;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod win_ffi;
#[cfg(target_os = "windows")]
pub mod win_wmi;
#[cfg(target_os = "windows")]
pub mod win_sensors;
#[cfg(target_os = "windows")]
pub mod win_dashboard;

use std::io::{self, Write};

pub struct CliOpts {
    pub once: bool,
    pub json: bool,
    pub interval: f64,
    pub no_ec: bool,
}

pub fn parse_cli(prog_name: &str, desc: &str) -> CliOpts {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = CliOpts {
        once: false,
        json: false,
        interval: 2.0,
        no_ec: false,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!(
                    "usage: {prog_name} [-h] [-V] [-1] [-w] [--json] [-n INTERVAL] [--no-ec]\n\n{desc}\n\noptions:\n  -h, --help            show this help message and exit\n  -V, --version         show program's version number and exit\n  -1, --once            print one snapshot and exit\n  -w, --watch           live view (default when attached to a terminal)\n  --json                print one reading as JSON and exit\n  -n, --interval SEC    refresh interval in seconds (default: 2.0)\n  --no-ec               skip ThinkPad EC read-only probe (L15 Windows build)"
                );
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("{prog_name} {}", ansi::VERSION);
                std::process::exit(0);
            }
            "-1" | "--once" => opts.once = true,
            "-w" | "--watch" => opts.once = false,
            "--json" => opts.json = true,
            "--no-ec" => opts.no_ec = true,
            "-n" | "--interval" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<f64>() {
                        opts.interval = v.max(0.2);
                    }
                    i += 1;
                }
            }
            other if other.starts_with("--interval=") => {
                if let Ok(v) = other["--interval=".len()..].parse::<f64>() {
                    opts.interval = v.max(0.2);
                }
            }
            _ => {}
        }
        i += 1;
    }
    opts
}

pub fn run_app(is_l15_windows: bool) {
    let prog = if is_l15_windows { "sensors-gorilla-l15" } else { "sensors-gorilla" };
    let desc = if is_l15_windows {
        "ThinkPad L15 Gen 3 (Alder Lake 2P+8E / Iris Xe / Micron NVMe) live sensor dashboard."
    } else {
        "Zero-dependency compiled hardware sensor dashboard. Live by default; follows window resize."
    };
    let opts = parse_cli(prog, desc);

    #[cfg(target_os = "windows")]
    {
        win_sensors::init_console();
        win_sensors::install_ctrl_c();
        let mut pdh_mon = win_sensors::PdhMonitor::new();
        if opts.json {
            let snap = win_sensors::collect(&mut pdh_mon, is_l15_windows, !opts.no_ec);
            println!("{}", win_dashboard::render_json(&snap, is_l15_windows));
            return;
        }
        let should_watch = win_sensors::stdout_isatty() && !opts.once;
        if !should_watch {
            let snap = win_sensors::collect(&mut pdh_mon, is_l15_windows, !opts.no_ec);
            println!("{}", win_dashboard::render(&snap, is_l15_windows));
            return;
        }
        let mut out = io::BufWriter::with_capacity(65_536, io::stdout());
        let _ = out.write_all(b"\x1b[?25l\x1b[2J\x1b[H");
        let mut last_size = (0usize, 0usize);
        while !win_sensors::ctrl_c_pressed() {
            let cur_size = win_sensors::terminal_size().unwrap_or((0, 0));
            let cols = cur_size.0;
            let snap = win_sensors::collect(&mut pdh_mon, is_l15_windows, !opts.no_ec);
            let frame = win_dashboard::render(&snap, is_l15_windows);
            let mut buf = String::with_capacity(frame.len() + 512);
            buf.push_str("\x1b[?2026h");
            if cur_size != last_size && last_size != (0, 0) {
                buf.push_str("\x1b[2J");
            }
            last_size = cur_size;
            buf.push_str("\x1b[H");
            for line in frame.lines() {
                buf.push_str(line);
                buf.push_str("\x1b[K\n");
            }
            let footer = format!(
                "Refreshed every {:.1}s{} · 24-bit TrueColor. Press Ctrl+C to exit.",
                opts.interval,
                if cols > 0 { format!(" · window {cols} cols, redraws on resize") } else { String::new() }
            );
            buf.push_str(&footer);
            buf.push_str("\x1b[K\x1b[J\x1b[?2026l");
            let _ = out.write_all(buf.as_bytes());
            let _ = out.flush();
            win_sensors::wait_resize_or_timeout((opts.interval * 1000.0) as u32);
        }
        let _ = out.write_all(b"\x1b[?25h\nExiting dashboard.\n");
        let _ = out.flush();
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = is_l15_windows;
        posix::install_term_handlers();
        if opts.json {
            #[cfg(target_os = "linux")]
            println!("{}", linux::build_json());
            #[cfg(target_os = "macos")]
            println!("{}", macos::build_json());
            return;
        }
        let should_watch = posix::stdout_isatty() && !opts.once;
        if !should_watch {
            #[cfg(target_os = "linux")]
            println!("{}", linux::build_dashboard());
            #[cfg(target_os = "macos")]
            println!("{}", macos::build_dashboard());
            return;
        }
        let waiter = posix::ResizeWaiter::new();
        let mut out = io::BufWriter::with_capacity(65_536, io::stdout());
        let _ = out.write_all(b"\x1b[?25l\x1b[2J\x1b[H");
        let mut last_size = (0usize, 0usize);
        while !posix::terminated() {
            let cur_size = posix::terminal_size().unwrap_or((0, 0));
            let cols = cur_size.0;
            #[cfg(target_os = "linux")]
            let frame = linux::build_dashboard();
            #[cfg(target_os = "macos")]
            let frame = macos::build_dashboard();
            let mut buf = String::with_capacity(frame.len() + 512);
            if cur_size != last_size && last_size != (0, 0) {
                buf.push_str("\x1b[2J");
            }
            last_size = cur_size;
            buf.push_str("\x1b[H");
            for line in frame.lines() {
                buf.push_str(line);
                buf.push_str("\x1b[K\n");
            }
            let footer = format!(
                "Refreshed every {:.1}s{}. Press Ctrl+C to exit.",
                opts.interval,
                if cols > 0 { format!(" · window {cols} cols, redraws on resize") } else { String::new() }
            );
            buf.push_str(&footer);
            buf.push_str("\x1b[K\x1b[J");
            let _ = out.write_all(buf.as_bytes());
            let _ = out.flush();
            let ms = (opts.interval * 1000.0) as i32;
            if let Some(ref w) = waiter {
                w.wait_ms(ms);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(ms as u64));
            }
        }
        let _ = out.write_all(b"\x1b[?25h\nExiting dashboard.\n");
        let _ = out.flush();
    }
}
