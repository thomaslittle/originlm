//! One notification coordinator for every consumer. Delivery state is durable
//! and duplicate suppression is best-effort across crashes; exactly-once OS
//! delivery cannot be guaranteed.
use chrono::{Local, Timelike};
use originlm_contracts::*;
use originlm_store::StateStore;

pub fn quiet_hours_active(preferences: &NotificationPreferences, minute: u16) -> bool {
    let (Some(start), Some(end)) = (
        preferences.quiet_hours_start_minute,
        preferences.quiet_hours_end_minute,
    ) else {
        return false;
    };
    if start == end {
        return true;
    }
    if start < end {
        (start..end).contains(&minute)
    } else {
        minute >= start || minute < end
    }
}

fn dedupe_key(issue: &Issue) -> String {
    format!("{}:{}", issue.id, severity_label(issue.severity))
}

fn severity_label(severity: IssueSeverity) -> &'static str {
    issue_severity_label(severity)
}

fn decision(
    issue: &Issue,
    should_notify: bool,
    reason: NotificationReason,
    preferences: &NotificationPreferences,
) -> NotificationDecision {
    let (title, body) = if preferences.private_content {
        (
            "OriginLM needs attention".to_owned(),
            "Open the control center to review a private local issue.".to_owned(),
        )
    } else {
        (issue.title.clone(), issue.summary.clone())
    };
    NotificationDecision {
        issue_id: issue.id.clone(),
        severity: issue.severity,
        title,
        body,
        should_notify,
        reason: reason.as_str().to_owned(),
        dedupe_key: dedupe_key(issue),
    }
}

/// Evaluate all unresolved issues. `store` is the single dedupe authority, so
/// two applications polling at once cannot both claim a new notification.
pub fn poll(
    issues: &[Issue],
    preferences: &NotificationPreferences,
    store: Option<&StateStore>,
    memory: &std::collections::BTreeMap<String, u64>,
    now_minute: u16,
    now_ms: u64,
) -> Vec<NotificationDecision> {
    let quiet = quiet_hours_active(preferences, now_minute);
    let mut decisions = Vec::new();
    for issue in issues
        .iter()
        .filter(|issue| issue.state != IssueState::Resolved && issue.state != IssueState::Snoozed)
    {
        if issue.severity < preferences.minimum_severity {
            continue;
        }
        if !preferences.enabled {
            decisions.push(decision(
                issue,
                false,
                NotificationReason::SuppressedDisabled,
                preferences,
            ));
            continue;
        }
        if issue
            .snoozed_until
            .as_ref()
            .is_some_and(|until| until.as_str() > Timestamp::now().as_str())
        {
            decisions.push(decision(
                issue,
                false,
                NotificationReason::SuppressedSnoozed,
                preferences,
            ));
            continue;
        }
        if quiet {
            decisions.push(decision(
                issue,
                false,
                NotificationReason::SuppressedQuietHours,
                preferences,
            ));
            continue;
        }
        let key = dedupe_key(issue);
        let previous = store
            .and_then(|store| {
                store
                    .record_notification_time(&issue.id, &key)
                    .ok()
                    .flatten()
            })
            .or_else(|| memory.get(&key).copied());
        let due = match previous {
            None => true,
            Some(at) => {
                now_ms.saturating_sub(at) >= preferences.reminder_after_seconds.saturating_mul(1000)
            }
        };
        if !due {
            decisions.push(decision(
                issue,
                false,
                NotificationReason::SuppressedDuplicate,
                preferences,
            ));
            continue;
        }
        let reason = if previous.is_some() {
            NotificationReason::Reminder
        } else {
            NotificationReason::NewIssue
        };
        if let Some(store) = store
            && store
                .record_notification(&issue.id, &key, severity_label(issue.severity), now_ms)
                .is_err()
        {
            // Persistence is the dedupe authority. Fail closed rather than
            // risk duplicate OS alerts from multiple consumers.
            decisions.push(decision(
                issue,
                false,
                NotificationReason::SuppressedDuplicate,
                preferences,
            ));
            continue;
        }
        decisions.push(decision(issue, true, reason, preferences));
    }
    decisions.truncate(64);
    decisions
}

pub fn local_minute() -> u16 {
    let now = Local::now();
    (now.hour() as u16) * 60 + now.minute() as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn issue(severity: IssueSeverity) -> Issue {
        let now = Timestamp::now();
        Issue {
            id: "issue_1".into(),
            fingerprint: "resource".into(),
            kind: IssueKind::AuthExpired,
            severity,
            state: IssueState::Open,
            title: "Sign-in expired".into(),
            summary: "private detail".into(),
            provider_id: None,
            connection_id: None,
            account_id: None,
            route_id: None,
            pool_id: None,
            window_id: None,
            first_seen_at: now.clone(),
            last_seen_at: now,
            resolved_at: None,
            snoozed_until: None,
            occurrences: 1,
            evidence: vec![],
            remediation: vec![],
            last_observation: None,
        }
    }

    #[test]
    fn duplicate_polls_do_not_emit_twice_and_reminders_are_bounded() {
        let directory = TempDir::new().unwrap();
        let store = originlm_store::StateStore::open(directory.path().join("state.db"))
            .unwrap()
            .0;
        let preferences = NotificationPreferences::default();
        let memory = std::collections::BTreeMap::new();
        let first = poll(
            &[issue(IssueSeverity::Error)],
            &preferences,
            Some(&store),
            &memory,
            12 * 60,
            1_000,
        );
        assert!(first[0].should_notify);
        let second = poll(
            &[issue(IssueSeverity::Error)],
            &preferences,
            Some(&store),
            &memory,
            12 * 60,
            2_000,
        );
        assert!(!second[0].should_notify);
        let later = poll(
            &[issue(IssueSeverity::Error)],
            &preferences,
            Some(&store),
            &memory,
            12 * 60,
            1_000 + preferences.reminder_after_seconds * 1000,
        );
        assert!(later[0].should_notify);
        assert_eq!(later[0].reason, "reminder");
    }

    #[test]
    fn quiet_hours_suppress_without_marking_delivered() {
        let preferences = NotificationPreferences {
            quiet_hours_start_minute: Some(22 * 60),
            quiet_hours_end_minute: Some(7 * 60),
            ..NotificationPreferences::default()
        };
        assert!(quiet_hours_active(&preferences, 23 * 60));
        assert!(quiet_hours_active(&preferences, 6 * 60));
        assert!(!quiet_hours_active(&preferences, 12 * 60));
        let decisions = poll(
            &[issue(IssueSeverity::Critical)],
            &preferences,
            None,
            &Default::default(),
            23 * 60,
            0,
        );
        assert!(!decisions[0].should_notify);
        assert_eq!(decisions[0].reason, "suppressed_quiet_hours");
    }

    #[test]
    fn private_content_hides_issue_text() {
        let preferences = NotificationPreferences {
            private_content: true,
            ..NotificationPreferences::default()
        };
        let decisions = poll(
            &[issue(IssueSeverity::Warning)],
            &preferences,
            None,
            &Default::default(),
            12 * 60,
            0,
        );
        assert!(!decisions[0].body.contains("private detail"));
        assert!(!decisions[0].title.contains("Sign-in"));
    }

    #[test]
    fn disabled_notifications_still_expose_inline_health_decisions() {
        let preferences = NotificationPreferences {
            enabled: false,
            ..NotificationPreferences::default()
        };
        let decisions = poll(
            &[issue(IssueSeverity::Warning)],
            &preferences,
            None,
            &Default::default(),
            12 * 60,
            0,
        );
        assert!(!decisions[0].should_notify);
        assert_eq!(decisions[0].reason, "suppressed_disabled");
    }
}
