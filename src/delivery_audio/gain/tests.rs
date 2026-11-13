use super::*;
use crate::audio_analysis::AudioScope;
use crate::audio_signal::{AudioSignalMeasurement, AudioSignalUnavailableReason, AudioSignalUnit};

fn configuration() -> Configuration {
    Configuration {schema:"aniflow.audio-gain-configuration/v1".into(),tools:serde_json::from_value(serde_json::json!({"schema":"aniflow.audio-inspection-configuration/v1","tool_timeout_milliseconds":1000,"maximum_tool_output_bytes":4096,"ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)}})).unwrap(),maximum_absolute_gain_milli_db:12_000,peak_guard_milli_db:200,maximum_output_bytes:16_777_216,maximum_working_bytes:1_073_741_824,minimum_free_bytes:0}
}
fn evaluation(loudness: f64, peak: f64) -> policy_check::Evaluation {
    let policy = super::super::policy::Policy {
        schema: super::super::policy::SCHEMA.into(),
        id: "synthetic".into(),
        revision: "1".into(),
        purpose: "Synthetic gain choice; no actual media".into(),
        target_milli_lufs: -14_000,
        tolerance_milli_lu: 500,
        maximum_true_peak_milli_dbtp: -1000,
    };
    let quantity = |unit, value| AudioSignalMeasurement {
        unit,
        scope: AudioScope {
            channels: vec![0, 1],
            stem_id: None,
        },
        value: AudioSignalValue::Measured { value },
    };
    policy_check::compare(
        &policy,
        2,
        &quantity(AudioSignalUnit::LoudnessUnitsFullScale, loudness),
        &quantity(AudioSignalUnit::DecibelsTruePeak, peak),
    )
    .unwrap()
}
#[test]
fn gain_requires_measured_headroom_and_does_not_invent_a_limiter() {
    let c = configuration();
    let ready = decide(&evaluation(-20.0, -9.0), &c).unwrap();
    assert_eq!(ready.state, DecisionState::Ready);
    assert_eq!(ready.gain_milli_db, Some(6000));
    assert_eq!(ready.predicted_true_peak_dbtp, Some(-3.0));
    assert_eq!(
        decide(&evaluation(-20.0, -2.0), &c).unwrap().state,
        DecisionState::PeakCeilingWouldBeExceeded
    );
    let mut bounded = c.clone();
    bounded.maximum_absolute_gain_milli_db = 1000;
    assert_eq!(
        decide(&evaluation(-20.0, -9.0), &bounded).unwrap().state,
        DecisionState::GainExceedsBound
    );
    assert_eq!(
        decide(&evaluation(-14.0, -2.0), &c).unwrap().state,
        DecisionState::AlreadyWithinPolicy
    );
}
#[test]
fn unavailable_or_forged_evaluation_flags_never_enable_processing() {
    let c = configuration();
    let mut check = evaluation(-20.0, -9.0);
    check.matches_selected_policy = true;
    assert!(decide(&check, &c).is_err());
    let mut check = evaluation(-20.0, -9.0);
    let mut loud = check.integrated_loudness.observed.clone();
    loud.value = AudioSignalValue::Unavailable {
        reason: AudioSignalUnavailableReason::SilentInput,
    };
    check = policy_check::compare(&check.policy, 2, &loud, &check.true_peak.observed).unwrap();
    assert_eq!(
        decide(&check, &c).unwrap().state,
        DecisionState::MissingEvidence
    );
}
#[test]
fn fixed_filter_preserves_signed_thousandths_without_shell_or_extra_processing() {
    assert_eq!(native::filter(1250), "volume=1.250dB:precision=double");
    assert_eq!(native::filter(-1), "volume=-0.001dB:precision=double");
    assert_eq!(native::filter(-24000), "volume=-24.000dB:precision=double");
}
