//! Source-format-preserving meter composition over existing #13 parsers.
//! This separately versioned receipt does not change native-v3's unavailable
//! meter contract or declare an unqualified algorithm qualified.
use std::ffi::OsString;
use std::path::Path;
use serde::{Deserialize,Serialize};
use crate::audio_analysis::AudioScope;
use crate::audio_inspection::{AudioInspectionConfiguration,NativeSampleFormat};
use crate::audio_inspection::process::{GroupPolicy,hash_regular,run_tool_isolated};
use crate::{CancellationToken,Error,ErrorCategory,Result};
use super::{AudioSignalMeasurement,AudioSignalUnit,AudioSignalValue,AudioSignalUnavailableReason};

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeLoudnessReceipt {
    pub schema:String,pub pcm_sha256:String,pub frames:u64,pub sample_rate_hz:u32,pub channels:u16,
    pub integrated_loudness:AudioSignalMeasurement,pub true_peak:AudioSignalMeasurement,
    pub method:String,pub commands:Vec<crate::audio_inspection::AudioTechnicalCommandEvidence>,
    pub runtime_qualified:bool,
}
fn invalid(message:&str)->Error {Error::new(ErrorCategory::Media,message)}

fn quantities(rate:u32,frames:u64,peaks:&[f64],integrated:f64,true_peak:Option<f64>)->Result<(AudioSignalMeasurement,AudioSignalMeasurement)> {
    if rate!=48_000||frames==0||!matches!(peaks.len(),1|2)||peaks.iter().any(|p|!p.is_finite()||*p<0.0)||!integrated.is_finite(){return Err(invalid("unsupported native loudness observation"));}
    let scope=AudioScope {channels:(0..peaks.len() as u16).collect(),stem_id:None};
    let peak=peaks.iter().copied().fold(0.0_f64,f64::max);
    let unavailable=|reason|AudioSignalValue::Unavailable {reason};
    let method=super::AudioSignalMethod::for_sample_rate_v2(rate);
    let integrated=if peak==0.0 {unavailable(AudioSignalUnavailableReason::SilentInput)}
        else if frames<method.integrated_minimum_frames {unavailable(AudioSignalUnavailableReason::InsufficientDuration)}
        else if integrated<=-70.0 {unavailable(AudioSignalUnavailableReason::BelowAbsoluteGate)}
        else {AudioSignalValue::Measured {value:integrated}};
    let true_peak=if peak==0.0 {
        if true_peak.is_some(){return Err(invalid("silent PCM returned finite true peak"));}
        unavailable(AudioSignalUnavailableReason::SilentInput)
    } else {
        let value=true_peak.filter(|p|p.is_finite()).ok_or_else(||invalid("nonzero PCM returned no finite true peak"))?;
        if value+f64::from(method.true_peak_sample_peak_tolerance_millidecibels)/1000.0<20.0*peak.log10(){return Err(invalid("true peak is below independently observed sample peak"));}
        AudioSignalValue::Measured {value}
    };
    Ok((AudioSignalMeasurement {unit:AudioSignalUnit::LoudnessUnitsFullScale,scope:scope.clone(),value:integrated},
        AudioSignalMeasurement {unit:AudioSignalUnit::DecibelsTruePeak,scope,value:true_peak}))
}

pub(crate) fn measure(path:&Path,expected:&(String,u64),configuration:&AudioInspectionConfiguration,
    directory:&Path,token:&CancellationToken)->Result<NativeLoudnessReceipt> {
    configuration.validate()?;
    if hash_regular(path,crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES).map_err(|_|invalid("PCM meter source unavailable"))?!=*expected {
        return Err(invalid("PCM meter source changed"));
    }
    let wave=crate::audio_inspection::wav::inspect_native(path)?;
    if wave.sample_format!=NativeSampleFormat::Float32||wave.sample_rate_hz!=48_000||!matches!(wave.channels,1|2){return Err(invalid("native loudness initially requires 48 kHz mono/stereo float32 without resampling or downmix"));}
    let samples=super::pcm::measure_native(path,&wave,0.0,100,1.0)?;
    let commands=super::provider::command_evidence(wave.sample_rate_hz);
    let mut captures=Vec::new();
    for command in &commands {
        let arguments:Vec<OsString>=command.arguments.iter().map(|v|if v=="{snapshot}" {path.as_os_str().to_owned()}else{v.into()}).collect();
        captures.push(run_tool_isolated(&configuration.ffmpeg,"ffmpeg",&arguments,configuration,token,GroupPolicy::Own,Some(directory))
            .map_err(|_|invalid("native loudness tool capture failed or exceeded its bound"))?);
    }
    let main=super::ebur128::summary(&captures[0].stderr)?;
    super::ebur128::short_term(&captures[0].stdout,wave.sample_rate_hz,wave.frame_count)?;
    if captures.len()!=2||!captures[1].stdout.is_empty(){return Err(invalid("native true-peak pass differs from the fixed meter profile"));}
    let peak=super::ebur128::summary(&captures[1].stderr)?.true_peak;
    let peaks=samples.channels.iter().map(|v|v.sample_peak_ratio).collect::<Vec<_>>();
    let (integrated_loudness,true_peak)=quantities(wave.sample_rate_hz,wave.frame_count,&peaks,main.integrated,peak)?;
    if hash_regular(path,crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES).map_err(|_|invalid("PCM meter source unavailable"))?!=*expected {
        return Err(invalid("PCM meter source changed during measurement"));
    }
    Ok(NativeLoudnessReceipt {schema:"aniflow.native-pcm-loudness/v1".into(),pcm_sha256:wave.pcm_sha256,
        frames:wave.frame_count,sample_rate_hz:wave.sample_rate_hz,channels:wave.channels,integrated_loudness,true_peak,
        method:"Existing #13 FFmpeg 8.0.1 ebur128 two-pass meter/parser, 48 kHz float32; independent native sample peak; no PCM16 quantization".into(),commands,runtime_qualified:false})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_and_short_inputs_keep_typed_unavailability() {
        let (l,p)=quantities(48_000,100,&[0.0],-70.0,None).unwrap();
        assert_eq!(l.value,AudioSignalValue::Unavailable {reason:AudioSignalUnavailableReason::SilentInput});
        assert_eq!(p.value,l.value);
        let (l,_)=quantities(48_000,100,&[0.5],-12.0,Some(-6.0)).unwrap();
        assert_eq!(l.value,AudioSignalValue::Unavailable {reason:AudioSignalUnavailableReason::InsufficientDuration});
    }
    #[test]
    fn finite_overs_are_retained_and_inconsistent_true_peak_is_refused() {
        let (_,p)=quantities(48_000,48_000,&[2.0,0.5],-3.0,Some(6.2)).unwrap();
        assert_eq!(p.value,AudioSignalValue::Measured {value:6.2});
        assert!(quantities(48_000,48_000,&[2.0],-3.0,Some(-3.0)).is_err());
        assert!(quantities(44_100,44_100,&[0.5],-3.0,Some(-3.0)).is_err());
    }
}
