use crate::error::InquisitorError;
use std::time::Duration;

/// Represents an amount of microseconds
pub struct Microseconds(pub f64);

impl std::fmt::Display for Microseconds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self.0 {
            x if x < 1_000.0 => write!(f, "{:.0} us", x),
            x if x < 10_000.0 => write!(f, "{:.2} ms", x / 1000.0),
            x if x < 100_000.0 => write!(f, "{:.1} ms", x / 1000.0),
            x if x < 1_000_000.0 => write!(f, "{:.0} ms", x / 1000.0),
            x if x < 10_000_000.0 => write!(f, "{:.2} s", x / 1_000_000.0),
            x if x < 100_000_000.0 => write!(f, "{:.1} s", x / 1_000_000.0),
            x if x < 1_000_000_000.0 => write!(f, "{:.0} s", x / 1_000_000.0),
            x => write!(f, "{:.0} s", x / 1_000_000.0),
        }
    }
}

/// Parse a duration like 10s, 25m, 3.5h into a duration. Decimal numbers are
/// allowed, and the allowed time units are: seconds (s), minutes (m) and
/// hours (h).
pub fn parse_duration(duration: &str) -> Result<Duration, InquisitorError> {
    let re = regex::Regex::new(r"(\d\d*(?:\.\d\d*)??)([smh])").expect("Bug: wrong regex");
    let cap = re
        .captures(duration)
        .ok_or(InquisitorError::DurationParseError)?;

    let base = cap[1]
        .parse::<f64>()
        .map_err(|_| InquisitorError::DurationParseError)?;
    let mul: f64 = match &cap[2] {
        "s" => 1_000_000.0,
        "m" => 60.0 * 1_000_000.0,
        "h" => 60.0 * 60.0 * 1_000_000.0,
        _ => unreachable!(),
    };

    Ok(Duration::from_micros((base * mul) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_time_correctly() {
        assert_eq!(Microseconds(0.0).to_string(), "0 us");
        assert_eq!(Microseconds(1.0).to_string(), "1 us");
        assert_eq!(Microseconds(10.0).to_string(), "10 us");
        assert_eq!(Microseconds(100.0).to_string(), "100 us");
        assert_eq!(Microseconds(999.0).to_string(), "999 us");
        assert_eq!(Microseconds(1000.0).to_string(), "1.00 ms");
        assert_eq!(Microseconds(1010.0).to_string(), "1.01 ms");
        assert_eq!(Microseconds(10_000.0).to_string(), "10.0 ms");
        assert_eq!(Microseconds(100_000.0).to_string(), "100 ms");
        assert_eq!(Microseconds(999_000.0).to_string(), "999 ms");
        assert_eq!(Microseconds(1_000_000.0).to_string(), "1.00 s");
        assert_eq!(Microseconds(10_000_000.0).to_string(), "10.0 s");
        assert_eq!(Microseconds(100_000_000.0).to_string(), "100 s");
    }
}
