//! Minimal terminal formatting: aligned tables and dimmed labels, without
//! colour beyond the terminal's own dim attribute.

use std::io::IsTerminal;

pub fn dim(s: &str) -> String {
    if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        format!("\x1b[2m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn kv(k: &str, v: &str) {
    println!("{}  {v}", dim(&format!("{k:<12}")));
}

pub fn table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(c.chars().count());
            }
        }
    }
    let line = |cells: Vec<String>| {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let pad = widths[i].saturating_sub(c.chars().count());
                if i + 1 == cells.len() { c.clone() } else { format!("{c}{}", " ".repeat(pad)) }
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!("{}", dim(&line(headers.iter().map(|s| s.to_string()).collect())));
    for r in rows {
        println!("{}", line(r.clone()));
    }
}

pub fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000_000 => format!("{:.1}B", n as f64 / 1e9),
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 10_000 => format!("{:.0}k", n as f64 / 1e3),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

pub fn duration_secs(s: i64) -> String {
    match s {
        s if s >= 86_400 => format!("{}d {}h", s / 86_400, (s % 86_400) / 3600),
        s if s >= 3600 => format!("{}h {}m", s / 3600, (s % 3600) / 60),
        s if s >= 60 => format!("{}m {}s", s / 60, s % 60),
        s => format!("{s}s"),
    }
}

pub fn relative_time(iso: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else { return iso.to_string() };
    let secs = (t.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_seconds();
    if secs >= 0 {
        format!("in {}", duration_secs(secs))
    } else {
        format!("{} ago", duration_secs(-secs))
    }
}

pub fn short_time(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|t| t.with_timezone(&chrono::Local).format("%b %d %H:%M:%S").to_string())
        .unwrap_or_else(|_| iso.to_string())
}
