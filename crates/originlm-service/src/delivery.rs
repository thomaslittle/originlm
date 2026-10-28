//! Optional OS notification delivery. The broker always computes deduplicated
//! decisions; a host must explicitly install a sink before anything is shown.
//! Tests inject a fake sink and never touch the desktop notification service.
use originlm_contracts::NotificationDecision;

pub trait NotificationSink: Send + Sync {
    fn deliver(&self, decision: &NotificationDecision) -> Result<(), String>;
}

/// Real desktop delivery for hosts that explicitly opt in at build and runtime.
/// No provider identity is added beyond the already-projected decision text.
#[cfg(feature = "os-notifications")]
pub struct OsNotificationSink;

#[cfg(feature = "os-notifications")]
impl NotificationSink for OsNotificationSink {
    fn deliver(&self, decision: &NotificationDecision) -> Result<(), String> {
        notify_rust::Notification::new()
            .appname("OriginLM")
            .summary(&decision.title)
            .body(&decision.body)
            .show()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use originlm_contracts::{IssueSeverity, NotificationDecision};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Recording(Mutex<Vec<NotificationDecision>>);

    impl NotificationSink for Recording {
        fn deliver(&self, decision: &NotificationDecision) -> Result<(), String> {
            self.0.lock().unwrap().push(decision.clone());
            Ok(())
        }
    }

    fn decision() -> NotificationDecision {
        NotificationDecision {
            issue_id: "issue".into(),
            severity: IssueSeverity::Warning,
            title: "Sign-in expired".into(),
            body: "private detail".into(),
            should_notify: true,
            reason: "new_issue".into(),
            dedupe_key: "issue:warning".into(),
        }
    }

    #[test]
    fn injected_sink_receives_only_the_projected_decision() {
        let sink = Arc::new(Recording::default());
        sink.deliver(&decision()).unwrap();
        let seen = sink.0.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].title, "Sign-in expired");
        assert_eq!(seen[0].body, "private detail");
    }
}
