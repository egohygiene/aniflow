//! Fixed native worker; v3 owns stage lifecycle and acceptance.
use super::*;
use crate::frame_io::{copy_pinned,require_absent,ensure_parent};
pub(crate) fn execute_invocation(invocation:&crate::ProviderInvocationRequest)->Result<()> {
    invocation.validate()?;
    let operation:CaptionOperation=serde_json::from_value(serde_json::to_value(&invocation.configuration.values).map_err(|_|invalid("invalid caption configuration"))?).map_err(|_|invalid("invalid closed caption operation"))?;
    operation.validate()?;let work=operation.work;
    if !cfg!(unix)||invocation.configuration!=operation.configuration()?||invocation.stage_id!=work.stage()||invocation.inputs.len()!=3||invocation.outputs.len()!=2 {
        return Err(invalid("invocation is outside the fixed Unix caption profile"));
    }
    let input=|port:&str,id:&str,kind:&str,role:crate::ArtifactRole,stream:crate::StreamRole|->Result<&Path>{
        let entries=invocation.inputs.iter().filter(|i|i.port==port).collect::<Vec<_>>();
        if entries.len()!=1{return Err(invalid("caption input is absent or ambiguous"));}let entry=entries[0];
        if entry.artifact_id!=id||entry.artifact_type!=kind||entry.artifact_role!=role||entry.stream_role!=Some(stream)||entry.kind!=crate::ArtifactKind::File{return Err(invalid("caption input contract differs"));}Ok(&entry.path)
    };
    let source=input("source","caption_source","video/mp4",crate::ArtifactRole::TemporalSource,crate::StreamRole::Video)?;
    let srt=input("srt","caption_srt","application/x-subrip",crate::ArtifactRole::TemporalComponent,crate::StreamRole::Subtitle)?;
    let font=input("font","caption_font","application/octet-stream",crate::ArtifactRole::Intermediate,crate::StreamRole::Attachment)?;
    let output=|port:&str,id:&str,kind:&str,role:crate::ArtifactRole,stream:crate::StreamRole,directory:bool|->Result<&Path>{
        let entries=invocation.outputs.iter().filter(|i|i.port==port).collect::<Vec<_>>();
        if entries.len()!=1{return Err(invalid("caption output absent or ambiguous"));}let entry=entries[0];
        if entry.artifact_id!=id||entry.artifact_type!=kind||entry.artifact_role!=role||entry.stream_role!=Some(stream)||entry.kind!=if directory {crate::ArtifactKind::Directory}else{crate::ArtifactKind::File}{return Err(invalid("caption output contract differs"));}Ok(&entry.path)
    };
    let media=output("media",work.artifact(),work.media_type(),if work==CaptionWork::Preview {crate::ArtifactRole::Intermediate}else{crate::ArtifactRole::CandidateMaster},crate::StreamRole::Video,work==CaptionWork::Preview)?;
    let evidence=output("evidence",work.evidence(),"application/vnd.aniflow.caption-report+json",crate::ArtifactRole::ValidationEvidence,crate::StreamRole::TimedMetadata,false)?;
    let parent=media.parent().ok_or_else(||invalid("caption output parent missing"))?;
    if evidence.parent()!=Some(parent)||media==evidence{return Err(invalid("caption outputs must be distinct siblings"));}
    let settings=&operation.request.configuration.settings;let token=CancellationToken::default();
    let renderer=std::env::current_exe().map_err(|_|invalid("cannot identify caption renderer"))?;
    if identity(&renderer,crate::video_assembly::MAXIMUM_TOOL_BYTES)?.0!=operation.renderer_sha256{return Err(invalid("caption renderer implementation changed after preview binding"));}
    for path in [media,evidence] {
        require_absent(path)?;
        for source in [source,srt,font,settings.ffmpeg.executable.as_path(),settings.ffprobe.executable.as_path(),renderer.as_path()] {
            if source.starts_with(path)||path.starts_with(source){return Err(invalid("caption output overlaps input bytes"));}
        }
    }
    let expected=[(operation.source.artifact.sha256.clone(),operation.source.artifact.byte_count),(operation.lyrics.srt_sha256.clone(),operation.lyrics.srt_byte_count),(operation.request.configuration.font.sha256.clone(),operation.font_byte_count)];
    let sources=[source,srt,font];let bounds=[settings.maximum_source_bytes,local::DOCUMENT_BYTES,FONT_BYTES];
    for i in 0..3 {if identity(sources[i],bounds[i])?!=expected[i]{return Err(invalid("caption input bytes changed"));}}
    ensure_parent(parent)?;crate::video_assembly::require_storage(parent,operation.working_bytes()?,settings.minimum_free_bytes)?;
    let tools=crate::video_assembly::observe_components(settings)?;
    let private=tempfile::Builder::new().prefix(".caption-native-").tempdir_in(parent).map_err(|_|invalid("cannot reserve private caption work"))?;
    let root=private.path().canonicalize().map_err(|_|invalid("cannot resolve private caption work"))?;
    let result:Result<()>=(||{
        let mut native=settings.clone();native.ffmpeg.executable=root.join("ffmpeg");native.ffprobe.executable=root.join("ffprobe");
        for (pin,destination,expected) in [(&settings.ffmpeg,&native.ffmpeg.executable,&tools[0]),(&settings.ffprobe,&native.ffprobe.executable,&tools[1])] {
            copy_pinned(&pin.executable,destination,expected,crate::video_assembly::MAXIMUM_TOOL_BYTES,&token)?;
            #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(destination,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot make caption tool executable"))?;}
        }
        let snapshots=[root.join("source.mp4"),root.join("captions.srt"),root.join("font.ttf")];
        for i in 0..3 {copy_pinned(sources[i],&snapshots[i],&expected[i],bounds[i],&token)?;}
        let candidate=root.join("captioned.mp4");
        let report=if work==CaptionWork::Preview {
            let frames=crate::video_assembly::media::captions::preview(&operation,&native,&snapshots[0],media,&root,&token)?;
            local::bytes(&CaptionPreviewReport {schema:"aniflow.caption-preview-report/v1".into(),operation:operation.clone(),provider_lock_sha256:invocation.provider_lock_sha256.clone(),context_sha256:operation.context()?,subject_sha256:canonical_sha256(&frames)?,frames,review_scope:PREVIEW_SCOPE.into(),native_qualified:false,creative_approval:"not_performed".into()})?
        } else {
            let observed=crate::video_assembly::media::captions::deliver(&operation,&native,&snapshots[0],&snapshots[1],&candidate,&root,&token)?;
            copy_pinned(&candidate,media,&(observed.output_sha256.clone(),observed.output_byte_count),settings.maximum_output_bytes,&token)?;
            local::bytes(&CaptionDeliveryReport {schema:"aniflow.caption-delivery-report/v1".into(),operation:operation.clone(),provider_lock_sha256:invocation.provider_lock_sha256.clone(),media:observed,review_scope:DELIVERY_SCOPE.into(),lyrics_video_audio_binding:"operator_declared; not established_by_PCM_to_compressed_audio_analysis".into(),native_qualified:false,creative_approval:"not_performed".into(),publication_authority:"none".into()})?
        };
        for i in 0..3 {if identity(sources[i],bounds[i])?!=expected[i]||identity(&snapshots[i],bounds[i])?!=expected[i]{return Err(invalid("caption input or private snapshot changed during rendering"));}}
        if crate::video_assembly::observe_components(settings)?!=tools||crate::video_assembly::observe_components(&native)?!=tools{return Err(invalid("caption tool bytes changed during rendering"));}
        local::write(evidence,&report)?;local::sync(parent)?;Ok(())
    })();
    match result {Ok(())=>Ok(()),Err(error)=>{let _retained=private.keep();Err(error)}}
}
