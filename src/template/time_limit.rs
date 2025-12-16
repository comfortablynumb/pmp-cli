//! Time limit utilities for environment expiration
//!
//! This module provides functions to parse duration strings, calculate expiration times,
//! and check if environments have expired based on their time limit configuration.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};

use super::metadata::TimeLimit;

/// Parse a duration string into seconds.
///
/// Supported formats:
/// - `s` - seconds (e.g., "30s")
/// - `m` - minutes (e.g., "5m")
/// - `h` - hours (e.g., "2h")
/// - `d` - days (e.g., "7d")
/// - `w` - weeks (e.g., "2w")
/// - Combined formats (e.g., "1d12h", "2w3d")
pub fn parse_duration(input: &str) -> Result<u64> {
    let mut total_seconds: u64 = 0;
    let mut current_num = String::new();

    for ch in input.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else if ch.is_ascii_alphabetic() {
            if current_num.is_empty() {
                return Err(anyhow!("Invalid duration format"));
            }

            let num: u64 = current_num.parse()?;
            current_num.clear();

            let multiplier = match ch {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86400,
                'w' => 604800,
                _ => return Err(anyhow!("Unknown unit '{}'. Use: s, m, h, d, w", ch)),
            };

            total_seconds += num * multiplier;
        } else if !ch.is_whitespace() {
            return Err(anyhow!("Invalid character: '{}'", ch));
        }
    }

    if !current_num.is_empty() {
        return Err(anyhow!(
            "Duration must include a unit (s, m, h, d, w)"
        ));
    }

    if total_seconds == 0 {
        return Err(anyhow!("Duration must be greater than 0"));
    }

    Ok(total_seconds)
}

/// Calculate the expiration DateTime from a TTL string and creation timestamp.
pub fn calculate_expiration(created_at: &DateTime<Utc>, ttl: &str) -> Result<DateTime<Utc>> {
    let seconds = parse_duration(ttl)?;
    let duration = Duration::seconds(seconds as i64);

    created_at
        .checked_add_signed(duration)
        .ok_or_else(|| anyhow!("Expiration time calculation overflow"))
}

/// Check if a time limit has expired.
///
/// Returns `Ok(true)` if expired, `Ok(false)` if not expired.
/// Returns an error if TTL is used but `created_at` is not provided.
pub fn is_expired(time_limit: &TimeLimit, created_at: Option<&DateTime<Utc>>) -> Result<bool> {
    let now = Utc::now();

    if let Some(expires_at) = &time_limit.expires_at {
        return Ok(now > *expires_at);
    }

    if let Some(ttl) = &time_limit.ttl {
        let created = created_at.ok_or_else(|| {
            anyhow!("TTL specified but created_at timestamp is missing")
        })?;

        let expiration = calculate_expiration(created, ttl)?;
        return Ok(now > expiration);
    }

    // No time limit configured
    Ok(false)
}

/// Get the expiration DateTime for a time limit configuration.
///
/// Returns `None` if no time limit is configured.
pub fn get_expiration(
    time_limit: &TimeLimit,
    created_at: Option<&DateTime<Utc>>,
) -> Result<Option<DateTime<Utc>>> {
    if let Some(expires_at) = &time_limit.expires_at {
        return Ok(Some(*expires_at));
    }

    if let Some(ttl) = &time_limit.ttl {
        let created = created_at.ok_or_else(|| {
            anyhow!("TTL specified but created_at timestamp is missing")
        })?;

        let expiration = calculate_expiration(created, ttl)?;
        return Ok(Some(expiration));
    }

    Ok(None)
}

/// Format the expiration status for display.
///
/// Returns a human-readable string like "2 days ago" or "in 3 hours".
pub fn format_expiration_status(
    time_limit: &TimeLimit,
    created_at: Option<&DateTime<Utc>>,
) -> Result<String> {
    let expiration = get_expiration(time_limit, created_at)?;

    match expiration {
        Some(exp) => {
            let now = Utc::now();
            let diff = exp.signed_duration_since(now);

            if diff.num_seconds() < 0 {
                Ok(format_duration_ago(-diff.num_seconds()))
            } else {
                Ok(format!("in {}", format_duration(diff.num_seconds())))
            }
        }
        None => Ok("no expiration".to_string()),
    }
}

/// Format seconds as a human-readable duration string (e.g., "2d 5h").
fn format_duration(total_seconds: i64) -> String {
    let seconds = total_seconds.abs();

    if seconds < 60 {
        return format!("{}s", seconds);
    }

    if seconds < 3600 {
        let minutes = seconds / 60;
        return format!("{}m", minutes);
    }

    if seconds < 86400 {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;

        if minutes > 0 {
            return format!("{}h {}m", hours, minutes);
        }
        return format!("{}h", hours);
    }

    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;

    if hours > 0 {
        return format!("{}d {}h", days, hours);
    }
    format!("{}d", days)
}

/// Format duration as "X ago" string.
fn format_duration_ago(seconds: i64) -> String {
    format!("{} ago", format_duration(seconds))
}

/// Validate a TimeLimit configuration.
///
/// Returns an error if both `expires_at` and `ttl` are set, or if neither is set.
pub fn validate_time_limit(time_limit: &TimeLimit) -> Result<()> {
    match (&time_limit.expires_at, &time_limit.ttl) {
        (Some(_), Some(_)) => Err(anyhow!(
            "Cannot specify both 'expires_at' and 'ttl' - choose one"
        )),
        (None, None) => Err(anyhow!(
            "Time limit must specify either 'expires_at' or 'ttl'"
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(parse_duration("30s").unwrap(), 30);
        assert_eq!(parse_duration("1s").unwrap(), 1);
    }

    #[test]
    fn test_parse_duration_minutes() {
        assert_eq!(parse_duration("5m").unwrap(), 300);
        assert_eq!(parse_duration("1m").unwrap(), 60);
    }

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(parse_duration("2h").unwrap(), 7200);
        assert_eq!(parse_duration("24h").unwrap(), 86400);
    }

    #[test]
    fn test_parse_duration_days() {
        assert_eq!(parse_duration("7d").unwrap(), 604800);
        assert_eq!(parse_duration("1d").unwrap(), 86400);
    }

    #[test]
    fn test_parse_duration_weeks() {
        assert_eq!(parse_duration("2w").unwrap(), 1209600);
        assert_eq!(parse_duration("1w").unwrap(), 604800);
    }

    #[test]
    fn test_parse_duration_combined() {
        assert_eq!(parse_duration("1d12h").unwrap(), 129600);
        assert_eq!(parse_duration("2w3d").unwrap(), 1468800);
        assert_eq!(parse_duration("1h30m").unwrap(), 5400);
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("5").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("5x").is_err());
        assert!(parse_duration("0d").is_err());
    }

    #[test]
    fn test_is_expired_with_expires_at_past() {
        let time_limit = TimeLimit {
            expires_at: Some(Utc::now() - Duration::hours(1)),
            ttl: None,
        };

        assert!(is_expired(&time_limit, None).unwrap());
    }

    #[test]
    fn test_is_expired_with_expires_at_future() {
        let time_limit = TimeLimit {
            expires_at: Some(Utc::now() + Duration::hours(1)),
            ttl: None,
        };

        assert!(!is_expired(&time_limit, None).unwrap());
    }

    #[test]
    fn test_is_expired_with_ttl_expired() {
        let created_at = Utc::now() - Duration::days(10);
        let time_limit = TimeLimit {
            expires_at: None,
            ttl: Some("7d".to_string()),
        };

        assert!(is_expired(&time_limit, Some(&created_at)).unwrap());
    }

    #[test]
    fn test_is_expired_with_ttl_not_expired() {
        let created_at = Utc::now() - Duration::days(3);
        let time_limit = TimeLimit {
            expires_at: None,
            ttl: Some("7d".to_string()),
        };

        assert!(!is_expired(&time_limit, Some(&created_at)).unwrap());
    }

    #[test]
    fn test_is_expired_with_ttl_missing_created_at() {
        let time_limit = TimeLimit {
            expires_at: None,
            ttl: Some("7d".to_string()),
        };

        assert!(is_expired(&time_limit, None).is_err());
    }

    #[test]
    fn test_validate_time_limit_expires_at_only() {
        let time_limit = TimeLimit {
            expires_at: Some(Utc::now()),
            ttl: None,
        };

        assert!(validate_time_limit(&time_limit).is_ok());
    }

    #[test]
    fn test_validate_time_limit_ttl_only() {
        let time_limit = TimeLimit {
            expires_at: None,
            ttl: Some("7d".to_string()),
        };

        assert!(validate_time_limit(&time_limit).is_ok());
    }

    #[test]
    fn test_validate_time_limit_both_set() {
        let time_limit = TimeLimit {
            expires_at: Some(Utc::now()),
            ttl: Some("7d".to_string()),
        };

        assert!(validate_time_limit(&time_limit).is_err());
    }

    #[test]
    fn test_validate_time_limit_neither_set() {
        let time_limit = TimeLimit {
            expires_at: None,
            ttl: None,
        };

        assert!(validate_time_limit(&time_limit).is_err());
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(30), "30s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(300), "5m");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(7200), "2h");
        assert_eq!(format_duration(5400), "1h 30m");
    }

    #[test]
    fn test_format_duration_days() {
        assert_eq!(format_duration(86400), "1d");
        assert_eq!(format_duration(129600), "1d 12h");
    }
}
