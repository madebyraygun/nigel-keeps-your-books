/// Format a float as a dollar amount with thousands separators: $1,234.56
pub fn money(val: f64) -> String {
    let negative = val < 0.0;
    let abs = val.abs();
    let cents = format!("{:.2}", abs);
    let parts: Vec<&str> = cents.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts[1];

    let mut with_commas = String::new();
    for (i, c) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(c);
    }
    let with_commas: String = with_commas.chars().rev().collect();

    if negative {
        format!("-${with_commas}.{dec_part}")
    } else {
        format!("${with_commas}.{dec_part}")
    }
}

/// Format a byte count as a human-readable string: "1.2 KB", "3.4 MB"
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_money_formatting() {
        assert_eq!(money(1234.56), "$1,234.56");
        assert_eq!(money(-500.00), "-$500.00");
        assert_eq!(money(0.0), "$0.00");
        assert_eq!(money(1000000.99), "$1,000,000.99");
        assert_eq!(money(42.10), "$42.10");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }
}
