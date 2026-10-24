pub mod json;
pub mod table;

use std::time::SystemTime;

pub fn fmt_size(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if b < K {
        format!("{bytes} B")
    } else if b < K * K {
        format!("{:.1}K", b / K)
    } else {
        format!("{:.1}M", b / (K * K))
    }
}

pub fn fmt_tokens(n: u64) -> String {
    if n < 1000 {
        format!("≈{n}")
    } else if n < 100_000 {
        format!("≈{:.1}k", n as f64 / 1000.0)
    } else {
        format!("≈{}k", n / 1000)
    }
}

/// "3d ago", "2h ago", "just now".
pub fn fmt_age(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "-".into() };
    let secs = SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (n, unit) = if secs < 60 {
        return "just now".into();
    } else if secs < 3600 {
        (secs / 60, "m")
    } else if secs < 86_400 {
        (secs / 3600, "h")
    } else if secs < 30 * 86_400 {
        (secs / 86_400, "d")
    } else if secs < 365 * 86_400 {
        (secs / (30 * 86_400), "mo")
    } else {
        (secs / (365 * 86_400), "y")
    };
    format!("{n}{unit} ago")
}
