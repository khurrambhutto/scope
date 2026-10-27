use crate::domain::operations::OperationResult;

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

    pub fn begin_apply(&mut self, plan_id: &str, updater_busy: bool) -> Option<u64> {
        if updater_busy
            || self.apply_request.is_some()
            || self.active_plan_id.as_deref() != Some(plan_id)
        {
            return None;
        }
        self.next_request += 1;
        self.apply_request = Some(self.next_request);
        Some(self.next_request)
    }

    pub fn accepts_apply(&self, request: u64) -> bool {
        self.apply_request == Some(request)
    }

    pub fn finish_apply(
        &mut self,
        request: u64,
        result: OperationResult,
    ) -> Option<OperationResult> {
        if !self.accepts_apply(request) {
            return None;
        }
        self.apply_request = None;
        self.active_plan_id = None;
        Some(result)
    }

    pub fn fail_apply(&mut self, request: u64) -> Option<OperationResult> {
        self.finish_apply(
            request,
            OperationResult {
                success: false,
                message: "Operation ended before Scope received a result. The system may have changed; Scope will rescan.".into(),
                logs: String::new(),
                exit_code: None,
            },
        )
    }

    pub fn can_start_updater(&self, updater_busy: bool) -> bool {
        !updater_busy && self.apply_request.is_none()
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
        controller.begin_apply("plan", false).unwrap();
        assert!(controller.begin_preview().is_none());
    }

    #[test]
    fn closing_a_dialog_does_not_cancel_a_background_apply() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("plan".into()));
        let apply = controller.begin_apply("plan", false).unwrap();

        controller.cancel_preview();

        assert!(controller.accepts_apply(apply));
        assert!(controller
            .finish_apply(apply, successful_result())
            .is_some());
        assert!(controller.begin_preview().is_some());
    }

    #[test]
    fn previous_apply_events_and_timer_ticks_cannot_affect_the_next_operation() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("first".into()));
        let first = controller.begin_apply("first", false).unwrap();
        controller.finish_apply(first, successful_result());
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("second".into()));
        let second = controller.begin_apply("second", false).unwrap();

        assert!(!controller.accepts_apply(first));
        assert!(controller
            .finish_apply(first, successful_result())
            .is_none());
        assert!(controller.accepts_apply(second));
    }

    #[test]
    fn failed_apply_result_is_preserved_when_the_request_finishes() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("plan".into()));
        let apply = controller.begin_apply("plan", false).unwrap();

        let result = controller.finish_apply(apply, failed_result()).unwrap();

        assert!(!result.success);
        assert!(result.message.contains("package manager"));
        assert!(!controller.is_applying());
    }

    #[test]
    fn apply_stream_ending_without_a_result_releases_the_operation() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("plan".into()));
        let apply = controller.begin_apply("plan", false).unwrap();

        let completion = controller.fail_apply(apply).unwrap();

        assert!(!completion.success);
        assert!(completion.message.contains("may have changed"));
        assert!(!controller.is_applying());
        assert!(controller.begin_preview().is_some());
    }

    #[test]
    fn package_apply_and_self_update_cannot_run_at_the_same_time() {
        let mut controller = OperationController::default();
        let preview = controller.begin_preview().unwrap();
        controller.accept_preview(preview, Some("plan".into()));
        assert!(controller.begin_apply("plan", true).is_none());
        assert!(!controller.is_applying());
        let apply = controller.begin_apply("plan", false).unwrap();
        assert!(!controller.can_start_updater(false));

        controller.finish_apply(apply, successful_result());
        assert!(controller.can_start_updater(false));
    }

    fn successful_result() -> OperationResult {
        OperationResult {
            success: true,
            message: String::new(),
            logs: String::new(),
            exit_code: Some(0),
        }
    }

    fn failed_result() -> OperationResult {
        OperationResult {
            success: false,
            message: "The package manager reported an error.".into(),
            logs: String::new(),
            exit_code: Some(1),
        }
    }
}
