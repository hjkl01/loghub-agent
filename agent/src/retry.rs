use std::time::Duration;

/// Calculate retry delay using exponential backoff.
///
/// retry_count=0 -> 1s
/// retry_count=1 -> 2s
/// retry_count=2 -> 4s
/// capped at 5 minutes.
pub fn retry_delay(retry_count: u32) -> Duration {
    let seconds = 2u64.saturating_pow(retry_count.min(8)) .min(300);
    Duration::from_secs(seconds)
}
