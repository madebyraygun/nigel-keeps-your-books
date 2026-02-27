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
}
