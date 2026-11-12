//! Read-only policy comparisons over current exact-final-file QC.
use serde::{Deserialize, Serialize};

use super::policy::Policy;
use crate::audio_signal::{AudioSignalMeasurement, AudioSignalUnit, AudioSignalValue};
use crate::destination::{cancelled, invalid, MediaFacts, MediaInput, MediaObservation};
use crate::{CancellationToken, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State { Matches, OutsideTarget, MissingEvidence }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub state: State,
    pub observed: AudioSignalMeasurement,
    pub minimum: Option<f64>,
    pub maximum: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    pub schema: String,
    pub policy: Policy,
    pub policy_sha256: String,
    pub integrated_loudness: Check,
    pub true_peak: Check,
    pub matches_selected_policy: bool,
    pub runtime_qualified: bool,
    pub creative_approval: String,
    pub publication_authority: String,
}

/// Pure comparison only. Call `assess` to establish current exact-file binding.
pub fn compare(policy: &Policy, channels: u16, loudness: &AudioSignalMeasurement,
               peak: &AudioSignalMeasurement) -> Result<Evaluation> {
    policy.validate()?;
    if !matches!(channels, 1 | 2) { return Err(invalid("audio policy needs a complete supported mono/stereo scope")); }
    for (measurement, unit) in [(loudness, AudioSignalUnit::LoudnessUnitsFullScale),
                                 (peak, AudioSignalUnit::DecibelsTruePeak)] {
        if measurement.unit != unit || measurement.scope.stem_id.is_some()
            || measurement.scope.channels != (0..channels).collect::<Vec<_>>()
            || matches!(&measurement.value, AudioSignalValue::Measured {value} if !value.is_finite())
        {
            return Err(invalid("audio policy measurement must be finite, correctly typed and cover all original channels"));
        }
    }
    let (low, high) = policy.loudness_bounds_milli_lufs()?;
    let check = |observed: &AudioSignalMeasurement, minimum: Option<f64>, maximum: f64| {
        let state = match &observed.value {
            AudioSignalValue::Unavailable {..} => State::MissingEvidence,
            AudioSignalValue::Measured {value}
                if *value <= maximum && minimum.is_none_or(|low| *value >= low) => State::Matches,
            _ => State::OutsideTarget,
        };
        Check {state, observed: observed.clone(), minimum, maximum}
    };
    let integrated_loudness = check(loudness, Some(f64::from(low) / 1000.0), f64::from(high) / 1000.0);
    let true_peak = check(peak, None, f64::from(policy.maximum_true_peak_milli_dbtp) / 1000.0);
    let matches_selected_policy = integrated_loudness.state == State::Matches && true_peak.state == State::Matches;
    Ok(Evaluation {schema: "aniflow.audio-policy-evaluation/v1".into(), policy: policy.clone(),
        policy_sha256: policy.identity_sha256()?, integrated_loudness, true_peak, matches_selected_policy,
        runtime_qualified: false, creative_approval: "not_performed".into(), publication_authority: "none".into()})
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request { pub schema: String, pub input: MediaInput, pub policy: Policy }

impl Request {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("audio policy check request exceeds 64 KiB")); }
        let request: Self = crate::provider::decode_json(bytes, "audio policy check request")?;
        request.validate()?; Ok(request)
    }
    pub fn validate(&self) -> Result<()> {
        self.policy.validate()?;
        if self.schema != "aniflow.audio-policy-check-request/v1"
            || !matches!(&self.input, MediaInput::AcceptedVideoWithQc {..} | MediaInput::AcceptedAudioWithQc {..})
        { return Err(invalid("audio policy checks require explicit current exact-final-file QC, not an earlier master meter")); }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assessment {
    pub schema: String,
    pub request: Request,
    pub subject: MediaObservation,
    pub evaluation: Evaluation,
    pub assessment_sha256: String,
}

pub fn assess(request: &Request, token: &CancellationToken) -> Result<Assessment> {
    request.validate()?; cancelled(token)?;
    let subject = crate::destination::observe_media(&request.input, token)?;
    let (channels, loudness, peak) = match &subject.facts {
        MediaFacts::VideoWithQc {qc, ..} => (qc.signal.channels, &qc.signal.loudness.integrated_loudness, &qc.signal.loudness.true_peak),
        MediaFacts::AudioWithQc {qc} => (qc.accepted_pcm.source.channels, &qc.integrated_loudness, &qc.true_peak),
        _ => return Err(invalid("audio policy observation lacks exact final-file QC")),
    };
    let evaluation = compare(&request.policy, channels, loudness, peak)?;
    let schema = "aniflow.audio-policy-assessment/v1".to_owned();
    let assessment_sha256 = crate::provider::canonical_sha256(&(&schema, request, &subject, &evaluation))?;
    if crate::destination::observe_media(&request.input, token)? != subject {
        return Err(invalid("final audio/QC changed during policy comparison"));
    }
    cancelled(token)?;
    Ok(Assessment {schema, request: request.clone(), subject, evaluation, assessment_sha256})
}

/// A saved comparison is usable only when the current owner reproduces it.
pub fn recheck(saved: &Assessment, token: &CancellationToken) -> Result<Assessment> {
    let current = assess(&saved.request, token)?;
    if &current != saved { return Err(invalid("saved audio policy comparison is stale or differs from current exact-file evidence")); }
    Ok(current)
}

/// Internal structural binding only; callers still need current owner readback.
pub(crate) fn validate_snapshot(saved:&Assessment)->Result<()> {
    saved.request.validate()?;
    let (channels,loudness,peak)=match &saved.subject.facts {
        MediaFacts::VideoWithQc {qc,..}=>(qc.signal.channels,&qc.signal.loudness.integrated_loudness,&qc.signal.loudness.true_peak),
        MediaFacts::AudioWithQc {qc}=>(qc.accepted_pcm.source.channels,&qc.integrated_loudness,&qc.true_peak),
        _=>return Err(invalid("policy snapshot lacks exact-final-file QC")),
    };
    if saved.schema!="aniflow.audio-policy-assessment/v1"||saved.request.input!=saved.subject.input
        ||compare(&saved.request.policy,channels,loudness,peak)?!=saved.evaluation
        ||saved.assessment_sha256!=crate::provider::canonical_sha256(&(&saved.schema,&saved.request,&saved.subject,&saved.evaluation))? {
        return Err(invalid("policy snapshot differs from its declared exact subject/settings/observations"));
    }Ok(())
}

#[cfg(test)]
mod tests;
