//! Per-run consent for paid or unknown-billing routes.
//!
//! The Playground switch arms consent for the model that is selected when the
//! user turns it on. Submitting a run consumes it: the run carries the captured
//! value and the switch is cleared, so a later turn never inherits it. Consent
//! armed for one model never applies to another, even before a run.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaidConsent {
    armed_for: Option<String>,
}

impl PaidConsent {
    /// Turn the switch on for `model`, or off.
    pub fn set(&mut self, model: Option<&str>, armed: bool) {
        self.armed_for = if armed {
            model.map(str::to_owned)
        } else {
            None
        };
    }

    /// Whether the switch shows as on for the currently selected model.
    pub fn is_armed_for(&self, model: Option<&str>) -> bool {
        model.is_some() && self.armed_for.as_deref() == model
    }

    /// Capture the value for the run being submitted and clear the switch.
    pub fn take_for_run(&mut self, model: &str) -> bool {
        self.armed_for.take().as_deref() == Some(model)
    }
}

#[cfg(test)]
mod tests {
    use super::PaidConsent;

    #[test]
    fn consent_is_off_by_default() {
        let mut consent = PaidConsent::default();
        assert!(!consent.is_armed_for(Some("a")));
        assert!(!consent.take_for_run("a"));
    }

    #[test]
    fn consent_applies_to_exactly_one_run() {
        let mut consent = PaidConsent::default();
        consent.set(Some("a"), true);
        assert!(consent.is_armed_for(Some("a")));
        assert!(consent.take_for_run("a"));
        assert!(!consent.is_armed_for(Some("a")));
        assert!(!consent.take_for_run("a"));
    }

    #[test]
    fn consent_does_not_follow_a_model_change() {
        let mut consent = PaidConsent::default();
        consent.set(Some("a"), true);
        assert!(!consent.is_armed_for(Some("b")));
        assert!(!consent.take_for_run("b"));
        // The mismatched run still cleared the switch.
        assert!(!consent.take_for_run("a"));
    }

    #[test]
    fn consent_needs_a_selected_model_and_can_be_withdrawn() {
        let mut consent = PaidConsent::default();
        consent.set(None, true);
        assert!(!consent.is_armed_for(None));
        consent.set(Some("a"), true);
        consent.set(Some("a"), false);
        assert!(!consent.take_for_run("a"));
    }
}
