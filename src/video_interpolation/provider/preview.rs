//! Selected frame work reuses the full provider's FrameRenderer and pair process.
use super::*;
use crate::video_interpolation::preview::{self as preview,PreviewConfiguration};
pub(crate) fn execute_preview_invocation(request:&ProviderInvocationRequest)->Result<()> {
    request.validate()?;
    let operation:PreviewConfiguration=serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|_|fail("invalid RIFE preview values"))?).map_err(|_|fail("invalid closed RIFE preview configuration"))?;
    operation.validate()?;
    if !cfg!(unix)||operation.configuration.host.operating_system!=std::env::consts::OS||operation.configuration.host.architecture!=std::env::consts::ARCH
        ||request.configuration!=operation.provider_configuration()?||request.stage_id!="rife_preview"||request.inputs.len()!=2||request.outputs.len()!=2{return Err(fail("invocation is outside the fixed Unix RIFE preview profile"));}
    let source_frames=selected(&request.inputs,"frames")?;let source_index=selected(&request.inputs,"index")?;
    let frames=selected(&request.outputs,"frames")?;let evidence=selected(&request.outputs,"index")?;
    for (actual,id,kind,directory,role,stream) in [
        (source_frames,"source_frames","application/vnd.aniflow.rife-frames",true,ArtifactRole::Intermediate,StreamRole::Video),
        (source_index,"source_index","application/vnd.aniflow.rife-frame-index+json",false,ArtifactRole::ValidationEvidence,StreamRole::TimedMetadata),
        (frames,"preview_frames","application/vnd.aniflow.rife-preview-frames",true,ArtifactRole::Intermediate,StreamRole::Video),
        (evidence,"preview_report","application/vnd.aniflow.rife-preview-report+json",false,ArtifactRole::ValidationEvidence,StreamRole::TimedMetadata),
    ] {
        if actual.artifact_id!=id||actual.artifact_type!=kind||actual.kind!=if directory {ArtifactKind::Directory}else{ArtifactKind::File}||actual.artifact_role!=role||actual.stream_role!=Some(stream){return Err(fail("RIFE preview port contract differs"));}
    }
    let parent=frames.path.parent().ok_or_else(||fail("RIFE preview output parent missing"))?;
    if evidence.path.parent()!=Some(parent)||frames.path==evidence.path{return Err(fail("RIFE preview outputs must be distinct siblings"));}
    let settings=&operation.configuration.settings;
    for output in [&frames.path,&evidence.path] {
        require_absent(output)?;
        for input in [&source_frames.path,&source_index.path,&settings.tool.executable,&settings.model.directory] {
            if output.starts_with(input)||input.starts_with(output){return Err(fail("RIFE preview output overlaps input/tool/model bytes"));}
        }
    }
    let token=CancellationToken::default();
    // Reuse complete original-index/frame admission on the actual v3 snapshots;
    // never rewrite their inspection or pass an excerpt off as the whole source.
    let mut snapshots=operation.clone();snapshots.frames_directory=source_frames.path.clone();snapshots.frame_index=source_index.path.clone();
    let input=preview::runtime::inputs(&snapshots,&token)?;
    let index=&input.index;let maximum=frame_byte_bound(index,settings)?;
    let mut report=preview::report_template(&operation,index,&input.index_identity,&input.directory.content_sha256,&request.provider_lock_sha256)?;
    let report_bytes=crate::provider::canonical_json_bytes(&report)?.len() as u64;
    if report_bytes>RIFE_MAXIMUM_INDEX_BYTES{return Err(fail("RIFE preview report exceeds its finite bound"));}
    let models=model_identities(settings,&settings.model.directory)?;
    verify_pin(&settings.tool,"rife-ncnn-vulkan").map_err(|_|dependency("RIFE preview tool differs from its pin"))?;
    let original_tool=identity(&settings.tool.executable,MAXIMUM_TOOL_BYTES)?;
    admit_storage(settings,index,input.source_bytes,original_tool.1,input.index_identity.1,report_bytes,report.frames.len() as u64)?;
    ensure_parent(parent)?;
    let workspace=tempfile::Builder::new().prefix(".rife-preview-").tempdir_in(parent).map_err(|_|fail("cannot reserve private RIFE preview work"))?;
    let root=workspace.path().canonicalize().map_err(|_|fail("cannot resolve private RIFE preview work"))?;
    let result:Result<()>=(||{
        let tool_directory=root.join("tool");let model_directory=root.join("rife-v4.6");let output_directory=root.join("frames");
        for directory in [&tool_directory,&model_directory,&output_directory] {std::fs::create_dir(directory).map_err(|_|fail("cannot reserve private RIFE preview directory"))?;}
        let mut tool=settings.tool.clone();tool.executable=tool_directory.join("rife-ncnn-vulkan");
        copy_pinned(&settings.tool.executable,&tool.executable,&original_tool,MAXIMUM_TOOL_BYTES,&token)?;
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&tool.executable,std::fs::Permissions::from_mode(0o700)).map_err(|_|dependency("cannot make private RIFE preview tool executable"))?;}
        for (path,expected,maximum) in &models {copy_pinned(path,&model_directory.join(path.file_name().expect("fixed model name")),expected,*maximum,&token)?;}
        let sources=index.frames.iter().map(|f|{let path=source_frames.path.join(&f.filename);let observed=identity(&path,maximum)?;Ok((path,observed))}).collect::<Result<Vec<_>>>()?;
        let renderer=FrameRenderer {sources:&sources,settings,width:index.width,height:index.height,tool:&tool,model_directory:&model_directory,output_directory:&output_directory,root:&root,cancellation:&token,maximum_frame_bytes:maximum};
        for frame in &mut report.frames {renderer.render(&mut frame.frame)?;}
        inventory(&tool_directory,&["rife-ncnn-vulkan".into()])?;private_workspace_inventory(&root)?;
        if identity(&settings.tool.executable,MAXIMUM_TOOL_BYTES)?!=original_tool||identity(&tool.executable,MAXIMUM_TOOL_BYTES)?!=original_tool||identity(&source_index.path,RIFE_MAXIMUM_INDEX_BYTES)?!=input.index_identity{return Err(fail("RIFE preview tool/index changed during rendering"));}
        model_identities(settings,&settings.model.directory)?;model_identities(settings,&model_directory)?;
        let current=preview::runtime::inputs(&snapshots,&token)?;
        if current.index!=*index||current.index_identity!=input.index_identity||current.directory!=input.directory{return Err(fail("RIFE preview complete original source changed during rendering"));}
        let names=report.frames.iter().map(|f|OsString::from(&f.frame.filename)).collect::<Vec<_>>();inventory(&output_directory,&names)?;
        for frame in &report.frames {if inspect_png(&output_directory.join(&frame.frame.filename),index.width,index.height,maximum)?.0!=frame.frame.sha256{return Err(fail("RIFE preview output changed before publication"));}}
        report.validate_against(index)?;let bytes=crate::provider::canonical_json_bytes(&report)?;
        if bytes.len() as u64!=report_bytes{return Err(fail("RIFE preview report differs from its exact reserved size"));}
        publish_json(&evidence.path,&bytes)?;require_absent(&frames.path)?;
        std::fs::rename(&output_directory,&frames.path).map_err(|_|fail("cannot publish complete RIFE preview sequence"))?;
        std::fs::File::open(parent).and_then(|f|f.sync_all()).map_err(|_|fail("cannot sync RIFE preview outputs"))?;Ok(())
    })();
    match result {Ok(())=>Ok(()),Err(error)=>{let _retained=workspace.keep();Err(error)}}
}
