//! Native spatial-input adapter over the shared full RIFE FrameRenderer.
use super::*;
use crate::video_interpolation::spatial::{self as spatial,Wrapper};
pub(crate) fn execute_spatial_invocation(request:&ProviderInvocationRequest)->Result<()> {
    request.validate()?;let wrapper:Wrapper=serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|_|fail("spatial RIFE configuration invalid"))?).map_err(|_|fail("closed spatial RIFE configuration invalid"))?;wrapper.validate()?;
    let configuration=&wrapper.configuration;let settings=&configuration.settings;
    if !cfg!(unix)||configuration.host.operating_system!=std::env::consts::OS||configuration.host.architecture!=std::env::consts::ARCH||request.configuration!=wrapper.provider_configuration()?||request.inputs.len()!=2||request.outputs.len()!=2{return Err(fail("spatial RIFE invocation differs from its fixed Unix profile"));}
    for (bindings,index_type) in [(&request.inputs,"application/vnd.aniflow.upscayl-output-index+json"),(&request.outputs,spatial::OUTPUT_TYPE)] {
        let frames=selected(bindings,"frames")?;let index=selected(bindings,"index")?;
        if frames.artifact_type!="application/vnd.aniflow.rife-frames"||frames.kind!=ArtifactKind::Directory||frames.artifact_role!=ArtifactRole::Intermediate||frames.stream_role!=Some(StreamRole::Video)
            ||index.artifact_type!=index_type||index.kind!=ArtifactKind::File||index.artifact_role!=ArtifactRole::ValidationEvidence||index.stream_role!=Some(StreamRole::TimedMetadata){return Err(fail("spatial RIFE port contract differs"));}
    }
    let source_frames=&selected(&request.inputs,"frames")?.path;let source_index=&selected(&request.inputs,"index")?.path;
    let frames_output=&selected(&request.outputs,"frames")?.path;let report_output=&selected(&request.outputs,"index")?.path;
    let current=spatial::observe_upscale(&wrapper.spatial.run_directory)?;
    if current.input.binding!=wrapper.spatial{return Err(fail("spatial RIFE accepted upstream binding changed"));}
    let raw:crate::upscayl_recovery::UpscaylOutputIndex=crate::destination::local::read_bounded(source_index,67_108_864)?;
    if raw!=current.input.index||identity(source_index,67_108_864)?.0!=wrapper.spatial.index_sha256{return Err(fail("spatial RIFE snapshot differs from accepted spatial index"));}
    let interpolation=spatial::plan(configuration,&raw)?;let maximum=frame_byte_bound_dimensions(raw.output_width,raw.output_height,settings)?;
    let mut report=crate::video_interpolation::RifeOutputIndex {schema:spatial::OUTPUT_SCHEMA.into(),frames:output_frame_template(&interpolation)?,plan:interpolation};
    let report_bytes=crate::provider::canonical_json_bytes(&report)?.len() as u64;
    if report_bytes>RIFE_MAXIMUM_INDEX_BYTES{return Err(fail("spatial RIFE evidence exceeds its finite limit"));}
    let names=raw.frames.iter().map(|f|OsString::from(&f.filename)).collect::<Vec<_>>();inventory(source_frames,&names)?;
    let sources=raw.frames.iter().map(|f|{let path=source_frames.join(&f.filename);let observed=inspect_png(&path,raw.output_width,raw.output_height,maximum)?;if observed.0!=f.sha256{return Err(fail("spatial RIFE snapshot PNG changed"));}Ok((path,observed))}).collect::<Result<Vec<_>>>()?;
    let models=model_identities(settings,&settings.model.directory)?;verify_pin(&settings.tool,"rife-ncnn-vulkan").map_err(|_|dependency("spatial RIFE tool changed"))?;let original_tool=identity(&settings.tool.executable,MAXIMUM_TOOL_BYTES)?;
    let total=sources.iter().try_fold(0u64,|n,(_,i)|n.checked_add(i.1).ok_or_else(||fail("spatial RIFE input byte overflow")))?;
    let needed=checked_sum(&[total,maximum.checked_mul(report.plan.output_frame_count+3).ok_or_else(||fail("spatial RIFE output byte overflow"))?,original_tool.1,settings.model.param_bytes,settings.model.bin_bytes,identity(source_index,67_108_864)?.1,report_bytes])?;
    if needed>settings.maximum_artifact_bytes{return Err(fail("spatial RIFE native admission exceeds selected bytes"));}
    let parent=frames_output.parent().ok_or_else(||fail("spatial RIFE output parent missing"))?;
    if report_output.parent()!=Some(parent)||frames_output==report_output{return Err(fail("spatial RIFE outputs must be distinct siblings"));}
    for output in [frames_output,report_output] {require_absent(output)?;for source in [source_frames,source_index,&settings.tool.executable,&settings.model.directory,&wrapper.spatial.run_directory] {if source.starts_with(output)||output.starts_with(source){return Err(fail("spatial RIFE outputs overlap input authority"));}}}
    ensure_parent(parent)?;let private=tempfile::Builder::new().prefix(".spatial-rife-").tempdir_in(parent).map_err(|_|fail("cannot reserve private spatial RIFE work"))?;let root=private.path().canonicalize().map_err(|_|fail("cannot resolve spatial RIFE work"))?;let token=CancellationToken::default();
    let result:Result<()>=(||{
        let tool_directory=root.join("tool");let model_directory=root.join("rife-v4.6");let frames=root.join("frames");
        for directory in [&tool_directory,&model_directory,&frames] {std::fs::create_dir(directory).map_err(|_|fail("cannot reserve spatial RIFE directory"))?;}
        let mut tool=settings.tool.clone();tool.executable=tool_directory.join("rife-ncnn-vulkan");copy_pinned(&settings.tool.executable,&tool.executable,&original_tool,MAXIMUM_TOOL_BYTES,&token)?;
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&tool.executable,std::fs::Permissions::from_mode(0o700)).map_err(|_|dependency("cannot make spatial RIFE tool executable"))?;}
        for (path,expected,bound) in &models {copy_pinned(path,&model_directory.join(path.file_name().expect("fixed model filename")),expected,*bound,&token)?;}
        let renderer=FrameRenderer {sources:&sources,settings,width:raw.output_width,height:raw.output_height,tool:&tool,model_directory:&model_directory,output_directory:&frames,root:&root,cancellation:&token,maximum_frame_bytes:maximum};
        for frame in &mut report.frames {renderer.render(frame)?;}
        inventory(&tool_directory,&["rife-ncnn-vulkan".into()])?;private_workspace_inventory(&root)?;model_identities(settings,&model_directory)?;model_identities(settings,&settings.model.directory)?;
        if identity(&tool.executable,MAXIMUM_TOOL_BYTES)?!=original_tool||identity(&settings.tool.executable,MAXIMUM_TOOL_BYTES)?!=original_tool||identity(source_index,67_108_864)?.0!=wrapper.spatial.index_sha256||spatial::observe_upscale(&wrapper.spatial.run_directory)?.input!=current.input{return Err(fail("spatial RIFE source/tools/upstream changed during use"));}
        for (path,expected) in &sources {if identity(path,maximum)?!=*expected{return Err(fail("spatial source snapshot changed"));}}
        let names=report.frames.iter().map(|f|OsString::from(&f.filename)).collect::<Vec<_>>();inventory(&frames,&names)?;
        for frame in &report.frames {if inspect_png(&frames.join(&frame.filename),raw.output_width,raw.output_height,maximum)?.0!=frame.sha256{return Err(fail("spatial RIFE output PNG changed"));}}
        spatial::validate_output(&report,configuration,&raw)?;let bytes=crate::provider::canonical_json_bytes(&report)?;
        if bytes.len() as u64!=report_bytes{return Err(fail("spatial RIFE reserved evidence size changed"));}
        publish_json(report_output,&bytes)?;std::fs::rename(&frames,frames_output).map_err(|_|fail("cannot publish spatial RIFE frames"))?;std::fs::File::open(parent).and_then(|f|f.sync_all()).map_err(|_|fail("cannot sync spatial RIFE outputs"))?;Ok(())
    })();
    match result {Ok(())=>Ok(()),Err(error)=>{let _retained=private.keep();Err(error)}}
}
