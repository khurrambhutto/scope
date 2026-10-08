#[derive(Default)]
pub(super) struct OperationController {
    next_request: u64,
    preview_request: Option<u64>,
    apply_request: Option<u64>,
    active_plan_id: Option<String>,
}

impl OperationController {
    pub fn begin_preview(&mut self) -> Option<u64> {
        if self.apply_request.is_some() {
            return None;
        }
        self.next_request += 1;
        self.preview_request = Some(self.next_request);
        self.active_plan_id = None;
        Some(self.next_request)
    }

    pub fn accept_preview(&mut self, request: u64, plan_id: Option<String>) -> bool {
        if self.preview_request != Some(request) {
            return false;
        }
        self.preview_request = None;
        self.active_plan_id = plan_id;
        true
    }

    pub fn cancel_preview(&mut self) {
        self.preview_request = None;
        if self.apply_request.is_none() {
            self.active_plan_id = None;
        }
    }

    pub fn begin_apply(&mut self, plan_id: &str) -> Option<u64> {
        if self.apply_request.is_some() || self.active_plan_id.as_deref() != Some(plan_id) {
            return None;
        }
        self.next_request += 1;
        self.apply_request = Some(self.next_request);
        Some(self.next_request)
    }

    pub fn accepts_apply(&self, request: u64) -> bool {
        self.apply_request == Some(request)
    }

    pub fn finish_apply(&mut self, request: u64) -> bool {
        if !self.accepts_apply(request) {
            return false;
        }
        self.apply_request = None;
        self.active_plan_id = None;
        true
    }

    pub fn is_applying(&self) -> bool {
        self.apply_request.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_preview_cannot_replace_the_active_plan() {
        let mut controller = OperationController::default();
        let first = controller.begin_preview().unwrap();
        let second = controller.begin_preview().unwrap();
        assert!(!controller.accept_preview(first, Some("old".into())));
        assert!(controller.accept_preview(second, Some("new".into())));
    }

    #[test]
    fn applying_blocks_a_second_destructive_operation() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("plan".into()));
        controller.begin_apply("plan").unwrap();
        assert!(controller.begin_preview().is_none());
    }

    #[test]
    fn closing_a_dialog_does_not_cancel_a_background_apply() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("plan".into()));
        let apply = controller.begin_apply("plan").unwrap();

        controller.cancel_preview();

        assert!(controller.accepts_apply(apply));
        assert!(controller.finish_apply(apply));
        assert!(controller.begin_preview().is_some());
    }

    #[test]
    fn previous_apply_events_and_timer_ticks_cannot_affect_the_next_operation() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("first".into()));
        let first = controller.begin_apply("first").unwrap();
        controller.finish_apply(first);
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("second".into()));
        let second = controller.begin_apply("second").unwrap();

        assert!(!controller.accepts_apply(first));
        assert!(!controller.finish_apply(first));
        assert!(controller.accepts_apply(second));
    }
}
