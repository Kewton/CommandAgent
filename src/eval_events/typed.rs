use std::path::Path;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct UltraPhaseEvent<'a> {
    event: &'a str,
    phase_id: &'a str,
    phase_index: usize,
    total_phases: usize,
    final_phase: bool,
    stage: &'a str,
    ok: Option<bool>,
    reason: String,
    step_count: Option<usize>,
}

impl<'a> UltraPhaseEvent<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        event: &'a str,
        phase_id: &'a str,
        index: usize,
        total_phases: usize,
        stage: &'a str,
        ok: Option<bool>,
        reason: Option<&str>,
        step_count: Option<usize>,
    ) -> Self {
        Self {
            event,
            phase_id,
            phase_index: index + 1,
            total_phases,
            final_phase: index + 1 == total_phases,
            stage,
            ok,
            reason: reason.map(super::body_snippet).unwrap_or_default(),
            step_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct PhaseVerificationEvent<'a> {
    event: &'static str,
    phase_id: &'a str,
    phase_index: usize,
    total_phases: usize,
    phase_verification_mode: &'a str,
    ok: bool,
    reason: String,
}

impl<'a> PhaseVerificationEvent<'a> {
    pub(crate) fn new(
        phase_id: &'a str,
        index: usize,
        total_phases: usize,
        mode: &'a str,
        ok: bool,
        reason: Option<&str>,
    ) -> Self {
        Self {
            event: "phase_verification_result",
            phase_id,
            phase_index: index + 1,
            total_phases,
            phase_verification_mode: mode,
            ok,
            reason: reason.map(super::body_snippet).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct UltraPlanCompleteEvent<'a> {
    event: &'static str,
    total_phases: usize,
    profile: &'a str,
    assurance_level: &'a str,
    assurance_reason: &'a str,
    ok: bool,
}

impl<'a> UltraPlanCompleteEvent<'a> {
    pub(crate) fn new(
        total_phases: usize,
        profile: &'a str,
        assurance_level: &'a str,
        assurance_reason: &'a str,
    ) -> Self {
        Self {
            event: "ultra_plan_complete",
            total_phases,
            profile,
            assurance_level,
            assurance_reason,
            ok: true,
        }
    }
}

pub(crate) fn emit(path: Option<&Path>, event: &impl Serialize) {
    match serde_json::to_value(event) {
        Ok(value) => super::emit(path, value),
        Err(err) => eprintln!("warning: failed to serialize typed eval event: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ultra_phase_event_preserves_legacy_shape() {
        let event = UltraPhaseEvent::new(
            "ultra_phase_complete",
            "phase-1",
            0,
            2,
            "complete",
            Some(true),
            None,
            Some(3),
        );
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            json!({
                "event": "ultra_phase_complete",
                "phase_id": "phase-1",
                "phase_index": 1,
                "total_phases": 2,
                "final_phase": false,
                "stage": "complete",
                "ok": true,
                "reason": "",
                "step_count": 3,
            })
        );
    }
}
