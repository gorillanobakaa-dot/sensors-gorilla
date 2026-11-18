//! 24-bit TrueColor rendering, Gorilla Skynet banner, thermal gradients, and width-adaptive layout.

use std::fmt::Write;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Thin box-drawing GORILLA · SKYNET · SYSTEM banner (3 lines × 79 visible cols).
pub const LOGO_RAW: [&str; 3] = [
    "╔═╗ ╔═╗ ╦═╗  ╦  ╦   ╦   ╔═╗   ╔═╗ ╦╔═ ╦ ╦ ╔╗╔ ╔══ ╔╦╗   ╔═╗ ╦ ╦ ╔═╗ ╔╦╗ ╔══ ╔╦╗",
    "║ ╦ ║ ║ ╠╦╝  ║  ║   ║   ╠═╣   ╚═╗ ╠╩╗ ╚╦╝ ║║║ ╠══  ║    ╚═╗ ╚╦╝ ╚═╗  ║  ╠══ ║║║",
    "╚═╝ ╚═╝ ╩╚═  ╩  ╚═╝ ╚═╝ ╩ ╩   ╚═╝ ╩ ╚  ╩  ╩ ╩ ╚══  ╩    ╚═╝  ╩  ╚═╝  ╩  ╚══ ╩ ╩",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorCap {
    TrueColor,
    Palette256,
}

/// Detect whether this terminal supports 24-bit RGB TrueColor (16.7M colors).
/// Windows Terminal / modern ConPTY, Linux terminals, and modern macOS terminals support TrueColor.
/// Legacy macOS Terminal.app (pre-10.15) without COLORTERM=truecolor uses xterm-256 quantization.
pub fn detect_color_cap() -> ColorCap {
    let term_prog = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let colorterm = std::env::var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
    if term_prog == "Apple_Terminal" && colorterm != "truecolor" && colorterm != "24bit" {
        ColorCap::Palette256
    } else {
        ColorCap::TrueColor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn lerp(self, other: Rgb, t: f64) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let r = (self.0 as f64 + (other.0 as f64 - self.0 as f64) * t).round() as u8;
        let g = (self.1 as f64 + (other.1 as f64 - self.1 as f64) * t).round() as u8;
        let b = (self.2 as f64 + (other.2 as f64 - self.2 as f64) * t).round() as u8;
        Rgb(r, g, b)
    }

    pub fn fg(self, cap: ColorCap, bold: bool) -> String {
        match cap {
            ColorCap::TrueColor => {
                if bold {
                    format!("\x1b[1;38;2;{};{};{}m", self.0, self.1, self.2)
                } else {
                    format!("\x1b[38;2;{};{};{}m", self.0, self.1, self.2)
                }
            }
            ColorCap::Palette256 => {
                let n = 16
                    + 36 * ((self.0 as f64 / 51.0).round() as u8)
                    + 6 * ((self.1 as f64 / 51.0).round() as u8)
                    + ((self.2 as f64 / 51.0).round() as u8);
                if bold {
                    format!("\x1b[1;38;5;{}m", n)
                } else {
                    format!("\x1b[38;5;{}m", n)
                }
            }
        }
    }
}

pub const RESET: &str = "\x1b[0m";

// Semantic TrueColor palette
pub const COL_HEADER: Rgb = Rgb(56, 189, 248);     // #38BDF8 sky cyan
pub const COL_SUBHEADER: Rgb = Rgb(125, 211, 252); // #7DD3FC ice blue
pub const COL_TREE: Rgb = Rgb(100, 116, 139);      // #64748B slate
pub const COL_MUTED: Rgb = Rgb(120, 134, 154);     // #78869A muted slate
pub const COL_BAR_EMPTY: Rgb = Rgb(48, 60, 78);    // #303C4E dark track
pub const COL_COOL: Rgb = Rgb(34, 229, 138);       // #22E58A emerald mint
pub const COL_LIME: Rgb = Rgb(132, 204, 22);       // #84CC16 lime
pub const COL_WARM: Rgb = Rgb(250, 204, 21);       // #FACC15 gold
pub const COL_ORANGE: Rgb = Rgb(251, 146, 60);     // #FB923C amber orange
pub const COL_HOT: Rgb = Rgb(255, 46, 76);         // #FF2E4C crimson hot
pub const COL_POWER: Rgb = Rgb(192, 132, 252);     // #C084FC violet
pub const COL_FREQ: Rgb = Rgb(34, 211, 238);       // #22D3EE electric cyan
pub const COL_WHITE: Rgb = Rgb(241, 245, 249);     // #F1F5F9 crisp white

/// Paint text with an RGB color (or 256-color quantized fallback).
pub fn paint(text: &str, color: Rgb, bold: bool, cap: ColorCap) -> String {
    format!("{}{}{}", color.fg(cap, bold), text, RESET)
}

/// Gorilla fire gradient across horizontal width: cyan (#00DCFF) -> orange (#FF8C00) -> crimson (#B41E3C).
pub fn fire_colour(x: usize, width: usize) -> Rgb {
    let t = (x as f64) / ((width.saturating_sub(1)).max(1) as f64);
    if t < 0.33 {
        let s = t / 0.33;
        Rgb(0, 220, 255).lerp(Rgb(255, 140, 0), s)
    } else if t < 0.66 {
        let s = (t - 0.33) / 0.33;
        Rgb(255, 140, 0).lerp(Rgb(220, 30, 0), s)
    } else {
        let s = (t - 0.66) / 0.34;
        Rgb(220, 30, 0).lerp(Rgb(180, 0, 60), s)
    }
}

/// Smooth continuous 24-bit thermal RGB ramp across [cool..warn..hot].
pub fn thermal_rgb(temp_c: f64, cool: f64, warn: f64, hot: f64) -> Rgb {
    if temp_c <= cool {
        COL_COOL
    } else if temp_c < warn {
        let t = (temp_c - cool) / (warn - cool).max(0.1);
        if t < 0.5 {
            COL_COOL.lerp(COL_LIME, t * 2.0)
        } else {
            COL_LIME.lerp(COL_WARM, (t - 0.5) * 2.0)
        }
    } else if temp_c < hot {
        let t = (temp_c - warn) / (hot - warn).max(0.1);
        COL_WARM.lerp(COL_ORANGE, t)
    } else {
        let t = ((temp_c - hot) / 15.0).clamp(0.0, 1.0);
        COL_ORANGE.lerp(COL_HOT, t)
    }
}

/// Smooth continuous RGB ramp for a 0..100% metric.
/// If `high_is_bad` is true (e.g. RAM/CPU usage), 0% = cool green -> 100% = crimson.
/// If `high_is_bad` is false (e.g. Battery/SSD health), 100% = cool green -> 0% = crimson.
pub fn pct_rgb(pct: f64, high_is_bad: bool) -> Rgb {
    let badness = if high_is_bad {
        (pct / 100.0).clamp(0.0, 1.0)
    } else {
        (1.0 - pct / 100.0).clamp(0.0, 1.0)
    };
    if badness < 0.5 {
        COL_COOL.lerp(COL_WARM, badness * 2.0)
    } else if badness < 0.8 {
        COL_WARM.lerp(COL_ORANGE, (badness - 0.5) / 0.3)
    } else {
        COL_ORANGE.lerp(COL_HOT, (badness - 0.8) / 0.2)
    }
}

/// Format a temperature in °C with smooth 24-bit thermal gradient coloring.
pub fn fmt_temp(v: Option<f64>, warn: f64, hot: f64, cap: ColorCap) -> String {
    match v {
        Some(t) => {
            let col = thermal_rgb(t, 42.0, warn, hot);
            paint(&format!("{:.1}°C", t), col, false, cap)
        }
        None => paint("N/A", COL_MUTED, false, cap),
    }
}

/// Render a smooth TrueColor gradient bar (e.g. `██████░░░░`) using millions of colors.
pub fn gradient_bar(pct: f64, width: usize, high_is_bad: bool, cap: ColorCap) -> String {
    let pct = pct.clamp(0.0, 100.0);
    let filled = ((pct / 100.0) * (width as f64)).round() as usize;
    let mut out = String::new();
    for i in 0..width {
        if i < filled {
            let pos_pct = if high_is_bad {
                ((i + 1) as f64 / width.max(1) as f64) * 100.0
            } else {
                pct
            };
            let c = pct_rgb(pos_pct, high_is_bad);
            let _ = write!(&mut out, "{}█", c.fg(cap, false));
        } else {
            let _ = write!(&mut out, "{}░", COL_BAR_EMPTY.fg(cap, false));
        }
    }
    out.push_str(RESET);
    out
}

/// Render the 3-line Gorilla Skynet System logo with cyan->orange->red fire gradient.
pub fn skynet_logo_lines(cap: ColorCap) -> Vec<String> {
    let max_w = LOGO_RAW.iter().map(|l| l.chars().count()).max().unwrap_or(79);
    LOGO_RAW
        .iter()
        .map(|line| {
            let mut out = String::with_capacity(line.len() * 16);
            for (i, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    out.push(' ');
                } else {
                    let c = fire_colour(i, max_w);
                    out.push_str(&c.fg(cap, true));
                    out.push(ch);
                }
            }
            out.push_str(RESET);
            out
        })
        .collect()
}

/// Render a horizontal TrueColor gradient separator bar (`====...====`).
pub fn gradient_rule(width: usize, ch: char, cap: ColorCap) -> String {
    let width = width.max(1);
    let mut out = String::with_capacity(width * 20);
    if cap == ColorCap::Palette256 {
        let _ = write!(&mut out, "{}", COL_HEADER.fg(cap, true));
        for _ in 0..width {
            out.push(ch);
        }
        out.push_str(RESET);
        return out;
    }
    // Smooth cyan -> sky -> violet -> amber accent along the rule
    let step = (width / 16).max(1);
    for i in 0..width {
        if i % step == 0 {
            let t = i as f64 / (width.saturating_sub(1)).max(1) as f64;
            let c = if t < 0.5 {
                Rgb(0, 210, 255).lerp(Rgb(168, 85, 247), t * 2.0)
            } else {
                Rgb(168, 85, 247).lerp(Rgb(255, 115, 36), (t - 0.5) * 2.0)
            };
            out.push_str(&c.fg(cap, true));
        }
        out.push(ch);
    }
    out.push_str(RESET);
    out
}

/// Visible character count of a string, ignoring ANSI CSI sequences (`\x1b[...m`) and `\x00` markers.
pub fn vis_len(s: &str) -> usize {
    let mut count = 0usize;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some(next) = chars.next() {
                if next == '[' {
                    for sc in chars.by_ref() {
                        if ('@'..='~').contains(&sc) {
                            break;
                        }
                    }
                }
            }
        } else if c != '\x00' {
            count += 1;
        }
    }
    count
}

/// Right-pad `s` with spaces up to `width` visible columns.
pub fn pad(s: &str, width: usize) -> String {
    let vl = vis_len(s);
    if vl >= width {
        s.to_string()
    } else {
        let mut out = String::with_capacity(s.len() + (width - vl));
        out.push_str(s);
        for _ in 0..(width - vl) {
            out.push(' ');
        }
        out
    }
}

/// Zip two column line lists side-by-side, ANSI-aware.
pub fn two_columns(left: &[String], right: &[String], gap: usize) -> Vec<String> {
    let rows = left.len().max(right.len());
    let lw = left.iter().map(|l| vis_len(l)).max().unwrap_or(0);
    let spacer = " ".repeat(gap);
    let mut out = Vec::with_capacity(rows);
    for i in 0..rows {
        let l = left.get(i).map(|s| s.as_str()).unwrap_or("");
        let r = right.get(i).map(|s| s.as_str()).unwrap_or("");
        let combined = format!("{}{}{}", pad(l, lw), spacer, r);
        out.push(combined.trim_end().to_string());
    }
    out
}

/// Assemble responsive two-column or stacked dashboard with banner, title, and footer rule.
pub fn assemble_dashboard(
    left: &[String],
    right: &[String],
    title_label: &str,
    term_cols: usize,
    term_rows: usize,
    watch_hint_cmd: &str,
    compact_for_short: bool,
    cap: ColorCap,
) -> String {
    let plan = |rows: &[String], tight: bool, gap: usize| -> (Vec<String>, usize, usize, usize) {
        let mapped: Vec<String> = rows
            .iter()
            .map(|l| {
                if tight {
                    l.split('\x00').next().unwrap_or("").trim_end().to_string()
                } else {
                    l.replace('\x00', "")
                }
            })
            .collect();
        let lw = left.iter().map(|l| vis_len(l)).max().unwrap_or(0);
        let rw = mapped.iter().map(|l| vis_len(l)).max().unwrap_or(0);
        (mapped, lw, rw, lw + gap + rw)
    };

    let (_, _, _, full_need) = plan(right, false, 3);
    let (_, _, _, tight_need) = plan(right, true, 1);

    let mut chosen: Option<(Vec<String>, usize, usize, usize, usize, bool)> = None;
    for (tight, gap) in [(false, 3), (true, 3), (true, 1)] {
        let (rows, lw, rw, need) = plan(right, tight, gap);
        if term_cols >= need {
            chosen = Some((rows, lw, rw, need, gap, tight));
            break;
        }
    }

    let (mut body, mut width) = if let Some((rows, _, _, need, gap, trimmed)) = chosen {
        let mut b = two_columns(left, &rows, gap);
        if trimmed {
            b.push(String::new());
            b.push(paint(
                &format!(
                    "Note: workload descriptions hidden to fit this {term_cols}-column terminal; showing them needs {full_need}."
                ),
                COL_MUTED,
                false,
                cap,
            ));
        }
        (b, need)
    } else {
        let (mut rows, lw, mut rw, _) = plan(right, false, 3);
        if term_cols < lw.max(rw) {
            let (t_rows, _, t_rw, _) = plan(right, true, 3);
            rows = t_rows;
            rw = t_rw;
        }
        let hints = [
            paint(
                &format!(
                    "Note: stacked - this terminal is {term_cols} cols; two columns need {tight_need} ({full_need} with descriptions)."
                ),
                COL_MUTED,
                false,
                cap,
            ),
            paint(
                &format!(
                    "      Resizing now will not reflow this output. Use '{watch_hint_cmd} -w' to follow the window."
                ),
                COL_MUTED,
                false,
                cap,
            ),
        ];
        let mut b = Vec::with_capacity(left.len() + rows.len() + 5);
        b.extend_from_slice(left);
        b.push(String::new());
        b.extend(rows);
        b.push(String::new());
        b.extend(hints.iter().cloned());
        let w = lw
            .max(rw)
            .max(hints.iter().map(|h| vis_len(h)).max().unwrap_or(0));
        (b, w)
    };

    if term_cols < 1_000_000 {
        width = width.min(term_cols).max(1);
    }

    let sep = gradient_rule(width, '=', cap);
    let centered_title = format!("{:^width$}", title_label, width = width);
    let title_line = paint(&centered_title, COL_HEADER, true, cap);

    if compact_for_short && (body.len() + 5 > term_rows) {
        body.retain(|l| !l.trim().is_empty());
        let mut out = vec![title_line];
        out.extend(body);
        return out.join("\n");
    }

    let logo = skynet_logo_lines(cap);
    let logo_w = LOGO_RAW.iter().map(|l| l.chars().count()).max().unwrap_or(79);
    let mut head = Vec::new();
    let show_logo = !compact_for_short || (width >= logo_w && body.len() + 3 + 1 + 3 + 1 <= term_rows);
    if show_logo {
        let indent = " ".repeat(width.saturating_sub(logo_w) / 2);
        for l in logo {
            head.push(format!("{indent}{l}"));
        }
    }
    head.push(sep.clone());
    head.push(title_line);
    head.push(sep.clone());

    let mut out = head;
    out.extend(body);
    out.push(sep);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vis_len_ignores_ansi_and_counts_unicode() {
        let colored = paint("├─ Core 0: 54.2°C", Rgb(34, 229, 138), true, ColorCap::TrueColor);
        assert_eq!(vis_len(&colored), "├─ Core 0: 54.2°C".chars().count());
    }

    #[test]
    fn fire_colour_stays_within_rgb_range() {
        let left = fire_colour(0, 79);
        let mid = fire_colour(39, 79);
        let right = fire_colour(78, 79);
        assert_eq!(left, Rgb(0, 220, 255));
        assert!(mid.0 > 200);
        assert!(right.0 >= 180 && right.2 >= 50);
    }

    #[test]
    fn two_columns_pads_left_ansi_correctly() {
        let l = vec![paint("A", COL_COOL, false, ColorCap::TrueColor), "BB".to_string()];
        let r = vec!["X".to_string(), "Y".to_string()];
        let out = two_columns(&l, &r, 3);
        assert_eq!(vis_len(&out[0]), 2 + 3 + 1);
        assert_eq!(vis_len(&out[1]), 2 + 3 + 1);
    }

    #[test]
    fn tight_marker_strips_descriptions_when_narrow() {
        let l = vec!["LEFT PANEL LINE WIDE ENOUGH 1234567890".to_string()];
        let r = vec!["RIGHT METRIC: 100 yrs\x00  long workload description text".to_string()];
        let rendered = assemble_dashboard(&l, &r, "TEST", 70, 40, "sensors-gorilla", false, ColorCap::TrueColor);
        assert!(!rendered.contains("long workload description text"));
        assert!(rendered.contains("RIGHT METRIC: 100 yrs"));
    }
}
