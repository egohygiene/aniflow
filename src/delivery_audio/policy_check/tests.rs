use super::*;
use crate::audio_analysis::AudioScope;
use crate::audio_signal::AudioSignalUnavailableReason;

fn policy() -> Policy {
    Policy {schema: super::super::policy::SCHEMA.into(), id: "synthetic".into(), revision: "1".into(),
        purpose: "Synthetic comparison, not a platform recommendation".into(),
        target_milli_lufs: -14_000, tolerance_milli_lu: 500, maximum_true_peak_milli_dbtp: -1000}
}
fn measured(unit: AudioSignalUnit, value: f64) -> AudioSignalMeasurement {
    AudioSignalMeasurement {unit, scope: AudioScope {channels: vec![0,1], stem_id: None}, value: AudioSignalValue::Measured {value}}
}

#[test]
fn exact_bounds_and_one_failing_quantity_control_the_result() {
    let p = policy(); let peak = measured(AudioSignalUnit::DecibelsTruePeak, -1.0);
    for value in [-14.5, -14.0, -13.5] {
        assert!(compare(&p,2,&measured(AudioSignalUnit::LoudnessUnitsFullScale,value),&peak).unwrap().matches_selected_policy);
    }
    for value in [-14.501,-13.499] {
        assert_eq!(compare(&p,2,&measured(AudioSignalUnit::LoudnessUnitsFullScale,value),&peak).unwrap().integrated_loudness.state,State::OutsideTarget);
    }
    let result=compare(&p,2,&measured(AudioSignalUnit::LoudnessUnitsFullScale,-14.0),&measured(AudioSignalUnit::DecibelsTruePeak,-0.99)).unwrap();
    assert!(!result.matches_selected_policy); assert_eq!(result.true_peak.state,State::OutsideTarget);
    assert!(!result.runtime_qualified); assert_eq!(result.creative_approval,"not_performed");
}

#[test]
fn unavailable_wrong_scope_wrong_units_and_nonfinite_are_never_passing() {
    let p=policy();let peak=measured(AudioSignalUnit::DecibelsTruePeak,-2.0);
    let mut loud=measured(AudioSignalUnit::LoudnessUnitsFullScale,-14.0);
    loud.value=AudioSignalValue::Unavailable {reason:AudioSignalUnavailableReason::SilentInput};
    let result=compare(&p,2,&loud,&peak).unwrap();
    assert_eq!(result.integrated_loudness.state,State::MissingEvidence);assert!(!result.matches_selected_policy);
    loud=measured(AudioSignalUnit::LoudnessUnitsFullScale,-14.0);loud.scope.channels=vec![0];assert!(compare(&p,2,&loud,&peak).is_err());
    loud=measured(AudioSignalUnit::Decibels,-14.0);assert!(compare(&p,2,&loud,&peak).is_err());
    loud=measured(AudioSignalUnit::LoudnessUnitsFullScale,f64::NAN);assert!(compare(&p,2,&loud,&peak).is_err());
}

#[test]
fn earlier_master_meter_is_refused_before_any_media_access() {
    let r=Request {schema:"aniflow.audio-policy-check-request/v1".into(),input:MediaInput::AcceptedPcm {file:"/synthetic/mix.wav".into(),signal_run_directory:"/synthetic/signal".into()},policy:policy()};
    assert!(r.validate().is_err());assert!(assess(&r,&CancellationToken::new()).is_err());
}
