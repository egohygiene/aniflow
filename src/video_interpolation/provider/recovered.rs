//! Recovered lineage admission over the shared native spatial render lifecycle.
use super::*;
use crate::video_interpolation::recovered::{self as recovered,Wrapper,RecoveredInput};
pub(crate) fn execute_recovered_invocation(request:&ProviderInvocationRequest)->Result<()> {
    request.validate()?;let wrapper:Wrapper=serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|_|fail("recovered RIFE configuration invalid"))?).map_err(|_|fail("closed recovered RIFE configuration invalid"))?;wrapper.validate()?;
    let configuration=&wrapper.configuration;
    if !cfg!(unix)||configuration.host.operating_system!=std::env::consts::OS||configuration.host.architecture!=std::env::consts::ARCH||request.configuration!=wrapper.provider_configuration()?||request.inputs.len()!=2||request.outputs.len()!=2{return Err(fail("recovered RIFE invocation differs from fixed native profile"));}
    for (bindings,index_type) in [(&request.inputs,recovered::INPUT_TYPE),(&request.outputs,recovered::OUTPUT_TYPE)] {
        let frames=selected(bindings,"frames")?;let index=selected(bindings,"index")?;
        if frames.artifact_type!="application/vnd.aniflow.rife-frames"||frames.kind!=ArtifactKind::Directory||frames.artifact_role!=ArtifactRole::Intermediate||frames.stream_role!=Some(StreamRole::Video)
            ||index.artifact_type!=index_type||index.kind!=ArtifactKind::File||index.artifact_role!=ArtifactRole::ValidationEvidence||index.stream_role!=Some(StreamRole::TimedMetadata){return Err(fail("recovered RIFE port contract differs"));}
    }
    let token=CancellationToken::default();let current=RecoveredInput::observe(&wrapper.recovered,&token)?;let source_index=&selected(&request.inputs,"index")?.path;
    let carrier:crate::upscayl_recovery::carrier::CompleteSpatialCarrier=crate::destination::local::read_bounded(source_index,67_108_864)?;
    if carrier!=current.carrier||identity(source_index,67_108_864)?.0!=wrapper.recovered.carrier_sha256{return Err(fail("recovered RIFE snapshot differs from exact current carrier"));}
    let interpolation=recovered::plan(configuration,&carrier)?;let frames=carrier.frames();
    super::spatial::render_bound(request,configuration,super::spatial::BoundRender {width:carrier.selection.request.output_width,height:carrier.selection.request.output_height,source_frames_spec:&frames,interpolation,output_schema:recovered::OUTPUT_SCHEMA,index_sha256:&wrapper.recovered.carrier_sha256,upstream:&wrapper.recovered.carrier_directory},
        |report|recovered::validate_output(report,configuration,&carrier),||{if RecoveredInput::observe(&wrapper.recovered,&token)?!=current{return Err(fail("recovered RIFE current origin authority changed"));}Ok(())})
}
