//! SSD Endurance & Real-World Lifetime Projections (shared across Linux and Windows).

use crate::ansi::{self, ColorCap};

#[derive(Clone, Debug)]
pub struct SsdProfile {
    pub display_name: String,
    pub rated_tbw: f64,
    pub capacity_gb: f64,
    pub dwpd_note: String,
    pub is_estimated: bool,
}

/// Resolve SSD endurance rating for a drive model + capacity + optional NVMe Percentage Used.
pub fn infer_profile(model: &str, capacity_bytes: u64, written_gb: Option<f64>, pct_used: Option<u8>) -> SsdProfile {
    let m = model.to_ascii_uppercase();
    if m.contains("DC600M") && (m.contains("1920") || capacity_bytes > 1_700_000_000_000) {
        return SsdProfile {
            display_name: "Kingston DC600M 1.92TB Enterprise".to_string(),
            rated_tbw: 3504.0,
            capacity_gb: 1920.0,
            dwpd_note: "3,504 TBW rated | 1 DWPD @ 5-yr".to_string(),
            is_estimated: false,
        };
    }
    if m.contains("MTFDKCD1T0TFK") {
        // Micron 2450 / 3400 1TB M.2 NVMe (ThinkPad L15 Gen 3 OEM SSD: 300 TBW rated)
        return SsdProfile {
            display_name: format!("{} (1TB PCIe Gen4 NVMe)", model.trim()),
            rated_tbw: 300.0,
            capacity_gb: 1024.0,
            dwpd_note: "300 TBW Micron spec | ~0.16 DWPD @ 5-yr".to_string(),
            is_estimated: false,
        };
    }
    let cap_gb = if capacity_bytes > 0 {
        (capacity_bytes as f64) / 1_000_000_000.0
    } else {
        1000.0
    };
    // If SMART Percentage Used >= 2% and we know written GB, derive actual controller TBW budget!
    if let (Some(w_gb), Some(u)) = (written_gb, pct_used) {
        if u >= 2 && w_gb > 100.0 {
            let derived_tbw = (w_gb / 1000.0) / (u as f64 / 100.0);
            if derived_tbw >= 50.0 && derived_tbw <= 20_000.0 {
                let rounded = (derived_tbw / 50.0).round() * 50.0;
                return SsdProfile {
                    display_name: if model.trim().is_empty() { "System SSD".to_string() } else { model.trim().to_string() },
                    rated_tbw: rounded,
                    capacity_gb: cap_gb,
                    dwpd_note: format!("~{:.0} TBW (from SMART {}% wear)", rounded, u),
                    is_estimated: true,
                };
            }
        }
    }
    let est_tbw = (cap_gb / 1000.0 * 600.0).max(150.0);
    SsdProfile {
        display_name: if model.trim().is_empty() { "System SSD".to_string() } else { model.trim().to_string() },
        rated_tbw: est_tbw,
        capacity_gb: cap_gb,
        dwpd_note: format!("~{:.0} TBW est. (600 TBW/TB TLC)", est_tbw),
        is_estimated: true,
    }
}

pub fn format_years(remaining_gb: f64, gb_per_day: f64) -> String {
    if gb_per_day <= 0.0 {
        return "∞".to_string();
    }
    let y = remaining_gb / gb_per_day / 365.25;
    if y >= 1000.0 {
        format_commas_f64(y, 0) + " yrs"
    } else {
        format!("{:.1} yrs", y)
    }
}

pub fn format_commas_f64(v: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v);
    let (int_part, frac_part) = match s.find('.') {
        Some(idx) => (&s[..idx], Some(&s[idx..])),
        None => (s.as_str(), None),
    };
    let mut out = String::new();
    let chars: Vec<char> = int_part.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 && *ch != '-' {
            out.push(',');
        }
        out.push(*ch);
    }
    if let Some(f) = frac_part {
        out.push_str(f);
    }
    out
}

/// Build the full `[STORAGE HEALTH & WORKLOAD]` + `[REMAINING LIFE — Real-World Projections]` lines.
pub fn build_projection_lines(
    profile: &SsdProfile,
    life_pct: Option<u8>,
    written_gb: Option<f64>,
    read_gb: Option<f64>,
    power_on_hours: Option<u64>,
    extra_lines: &[String],
    cap: ColorCap,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(ansi::paint(
        &format!("[STORAGE HEALTH & WORKLOAD ({})]", profile.display_name),
        ansi::COL_HEADER,
        true,
        cap,
    ));

    let life_str = match life_pct {
        Some(pct) => {
            let col = ansi::pct_rgb(pct as f64, false);
            let bar = ansi::gradient_bar(pct as f64, 10, false, cap);
            format!("{} {}%", bar, ansi::paint(&pct.to_string(), col, true, cap))
        }
        None => ansi::paint("N/A", ansi::COL_MUTED, false, cap),
    };
    lines.push(format!(
        "  SSD Remaining Life (Health) : {}  ({})",
        life_str,
        ansi::paint(&profile.dwpd_note, ansi::COL_MUTED, false, cap)
    ));

    let w_str = match (written_gb, read_gb) {
        (Some(w), Some(r)) => format!(
            "{} GB written  |  {} GB read",
            format_commas_f64(w, 2),
            format_commas_f64(r, 2)
        ),
        (Some(w), None) => format!("{} GB", format_commas_f64(w, 2)),
        _ => "N/A".to_string(),
    };
    lines.push(format!("  Total Lifetime Writes       : {w_str}"));

    let poh_str = match power_on_hours {
        Some(h) => format!("{} hrs ({:.1} days)", format_commas_f64(h as f64, 0), h as f64 / 24.0),
        None => "N/A".to_string(),
    };
    lines.push(format!("  Power-On Hours              : {poh_str}"));
    lines.extend(extra_lines.iter().cloned());

    let tbw_total_gb = profile.rated_tbw * 1000.0;
    let w_gb = written_gb.unwrap_or(0.0);
    let remaining_gb = (tbw_total_gb - w_gb).max(0.0);

    lines.push(String::new());
    lines.push(format!(
        "  {}",
        ansi::paint("[REMAINING LIFE — Real-World Projections]", ansi::COL_SUBHEADER, true, cap)
    ));
    lines.push(format!(
        "  Remaining rated endurance   : {:.1} TB  ({} GB left to TBW limit)",
        remaining_gb / 1000.0,
        format_commas_f64(remaining_gb, 0)
    ));
    lines.push(String::new());

    let c1 = ansi::paint("[1] NORMAL / LIGHT USE", ansi::COL_FREQ, true, cap);
    lines.push(format!("  {c1}  (browsing, YouTube, email, office)"));
    for (name, rate, desc) in [
        ("Light User     ", 2.0, "OS logs, occasional downloads"),
        ("Home User      ", 5.0, "Web browsing, YouTube, email"),
        ("Power Casual   ", 10.0, "Gaming, streaming, heavy browse"),
    ] {
        let yrs = format_years(remaining_gb, rate);
        let y_col = if remaining_gb / rate / 365.25 >= 10.0 { ansi::COL_COOL } else { ansi::COL_WARM };
        lines.push(format!(
            "  {} {} (~{:>2} GB/day)   : {:<12}\x00  {}",
            ansi::paint("├─", ansi::COL_TREE, false, cap),
            ansi::paint(name, ansi::COL_FREQ, false, cap),
            rate as u32,
            ansi::paint(&format!("{:<12}", yrs), y_col, false, cap),
            desc
        ));
    }
    // Fix last branch glyph to └─
    if let Some(last) = lines.last_mut() {
        *last = last.replacen("├─", "└─", 1);
    }

    lines.push(String::new());
    let c2 = ansi::paint("[2] GORILLA DEVELOPER HARDCORE", ansi::COL_WARM, true, cap);
    lines.push(format!("  {c2}  (compiling, ingesting, AI workloads)"));
    for (i, (name, rate, desc)) in [
        ("Light Dev      ", 5.0, "Normal coding + docs + OS churn"),
        ("Dev Average    ", 18.0, "2x weekly browser/kernel builds + AI"),
        ("Full Throttle  ", 60.0, "Daily browser + kernel builds + logs"),
        ("Absolute Max   ", 120.0, "Compiles + 4K encode + vector DB churn"),
    ]
    .iter()
    .enumerate()
    {
        let branch = if i == 3 { "└─" } else { "├─" };
        let yrs = format_years(remaining_gb, *rate);
        let y_col = if remaining_gb / rate / 365.25 >= 5.0 { ansi::COL_COOL } else { ansi::COL_WARM };
        lines.push(format!(
            "  {} {} (~{:>3} GB/day)  : {:<12}\x00  {}",
            ansi::paint(branch, ansi::COL_TREE, false, cap),
            ansi::paint(name, ansi::COL_WARM, false, cap),
            *rate as u32,
            ansi::paint(&format!("{:<12}", yrs), y_col, false, cap),
            desc
        ));
    }

    lines.push(String::new());
    let dwpd1 = profile.capacity_gb;
    let dwpd3 = profile.capacity_gb * 3.0;
    let c3 = ansi::paint("[3] HEAVY / ENTERPRISE BLAST", ansi::COL_HOT, true, cap);
    lines.push(format!("  {c3}"));
    for (i, (name, rate, desc)) in [
        ("Mixed Use Ent  ", 200.0, "DB servers, virtualisation, cloud".to_string()),
        ("1 DWPD Rated   ", dwpd1, format!("1 full drive wipe/day ({} GB/day)", format_commas_f64(dwpd1, 0))),
        ("Write Intense  ", dwpd3, format!("3 DWPD — extreme HPC ({} GB/day)", format_commas_f64(dwpd3, 0))),
    ]
    .iter()
    .enumerate()
    {
        let branch = if i == 2 { "└─" } else { "├─" };
        let yrs = format_years(remaining_gb, *rate);
        let y_est = remaining_gb / rate / 365.25;
        let y_col = if y_est >= 5.0 { ansi::COL_WARM } else { ansi::COL_HOT };
        lines.push(format!(
            "  {} {} (~{:>5} GB/day): {:<11}\x00  {}",
            ansi::paint(branch, ansi::COL_TREE, false, cap),
            ansi::paint(name, ansi::COL_HOT, false, cap),
            format_commas_f64(*rate, 0),
            ansi::paint(&format!("{:<11}", yrs), y_col, false, cap),
            desc
        ));
    }

    lines.push(String::new());
    lines.push(ansi::paint(
        "  Note: Projections from SMART wear/TBW counters. WAF not accounted for.",
        ansi::COL_MUTED,
        false,
        cap,
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kingston_and_micron_profiles_match_specs() {
        let k = infer_profile("KINGSTON SEDC600M1920G", 1_920_383_410_176, Some(490.75), Some(0));
        assert_eq!(k.rated_tbw, 3504.0);
        let m = infer_profile("Micron MTFDKCD1T0TFK", 1_024_203_640_320, Some(18746.05), Some(2));
        assert_eq!(m.rated_tbw, 300.0);
    }

    #[test]
    fn years_formatting_handles_large_and_small() {
        assert_eq!(format_years(3_503_509.25, 5.0), "1,918 yrs");
        assert_eq!(format_years(281_254.0, 18.0), "42.8 yrs");
    }
}
