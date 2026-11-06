//! Clock used for relative times in the UI.
//!
//! Screenshot capture pins `ORIGINLM_CAPTURE_TIME` to a Unix timestamp so
//! "Updated 2s" and reset countdowns do not drift between two captures of the
//! same screen. Unset (the normal case), every helper returns the real time.

use std::sync::OnceLock;

fn fixed() -> Option<i64> {
    static FIXED: OnceLock<Option<i64>> = OnceLock::new();
    *FIXED.get_or_init(|| {
        std::env::var("ORIGINLM_CAPTURE_TIME")
            .ok()
            .and_then(|value| value.trim().parse().ok())
    })
}

pub fn utc_now() -> chrono::DateTime<chrono::Utc> {
    fixed()
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .unwrap_or_else(chrono::Utc::now)
}

pub fn timestamp() -> i64 {
    utc_now().timestamp()
}

pub fn timestamp_millis() -> i64 {
    utc_now().timestamp_millis()
}
