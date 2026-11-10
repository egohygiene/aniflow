use super::*;
use std::ffi::OsString;
use crate::frame_io::{copy_pinned,inspect_png,inventory,require_absent};
use crate::audio_inspection::process::{GroupPolicy,run_tool_isolated_with_limits};
fn strings(values:&[&str])->Vec<OsString>{values.iter().map(|v|OsString::from(*v)).collect()}
fn period(op:&Operation)->Result<crate::temporal::RationalTime>{crate::temporal::RationalTime::new(op.artifact.frame_rate.denominator() as i64,op.artifact.frame_rate.numerator() as u64).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))}
pub(super) fn filter(op:&Operation,after:bool)->Result<String>{let r=&op.request;let end=r.first_frame.checked_add(r.frame_count).ok_or_else(||invalid("comparison range overflow"))?;let mut s=format!("trim=start_frame={}:end_frame={end},setpts=PTS-STARTPTS",r.first_frame);
    if after {let c=r.configuration.as_ref().ok_or_else(||invalid("deflicker not selected"))?;s.push_str(&format!(",deflicker=size={}:mode={}",c.window,if c.mode==Mode::Mean{"am"}else{"median"}));}
    s.push_str(",scale=in_range=tv:out_range=full:in_color_matrix=bt709:out_color_matrix=bt709,format=pix_fmts=rgb24,setsar=1");Ok(s)
}
fn arguments(op:&Operation,input:&Path,output:&Path,after:bool)->Result<Vec<OsString>> {
    let mut a=strings(&["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-noautorotate","-i"]);a.push(input.as_os_str().to_owned());
    a.extend(strings(&["-map","0:v:0","-an","-sn","-dn","-map_metadata","-1","-map_chapters","-1","-vf"]));a.push(filter(op,after)?.into());
    a.extend(strings(&["-frames:v"]));a.push(op.request.frame_count.to_string().into());a.extend(strings(&["-fps_mode:v","passthrough","-c:v","png","-pix_fmt","+rgb24","-threads:v","1","-start_number","0","-f","image2","-n"]));a.push(output.join("%08d.png").as_os_str().to_owned());Ok(a)
}
fn invoke(c:&AssemblyConfiguration,args:&[OsString],root:&Path,token:&CancellationToken)->Result<Vec<u8>> {let r=run_tool_isolated_with_limits(&c.ffmpeg,"ffmpeg",args,token,GroupPolicy::Inherit,Some(root),c.timeout_milliseconds,c.maximum_capture_bytes as u64).map_err(|_|invalid("deflicker native process failed/cancelled/exceeded bound"))?;if !r.stderr.is_empty(){return Err(invalid("deflicker process returned unexpected diagnostics"));}Ok(r.stdout)}
fn rgb(directory:&Path,op:&Operation,c:&AssemblyConfiguration,root:&Path,token:&CancellationToken)->Result<Vec<String>> {
    let mut a=strings(&["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-framerate"]);a.push(op.artifact.frame_rate.to_string().into());a.extend(strings(&["-start_number","0","-i"]));a.push(directory.join("%08d.png").as_os_str().to_owned());a.extend(strings(&["-map","0:v:0","-an","-sn","-dn","-vf","format=pix_fmts=rgb24","-c:v","rawvideo","-pix_fmt","+rgb24","-threads:v","1","-fps_mode:v","passthrough","-enc_time_base:v"]));a.push(period(op)?.to_string().into());a.extend(strings(&["-frames:v"]));a.push(op.request.frame_count.to_string().into());a.extend(strings(&["-f","framehash","-format_version","2","-hash","SHA256","pipe:1"]));
    parse_hashes(&invoke(c,&a,root,token)?,op)
}
pub(super) fn parse_hashes(bytes:&[u8],op:&Operation)->Result<Vec<String>> {
    let input=std::str::from_utf8(bytes).map_err(|_|invalid("comparison framehash is not UTF-8"))?;let mut tb=None;let mut hashes=Vec::new();let p=period(op)?;
    for required in ["#format: frame checksums".to_owned(),"#version: 2".to_owned(),"#hash: SHA256".to_owned(),"#media_type 0: video".to_owned(),"#codec_id 0: rawvideo".to_owned(),format!("#dimensions 0: {}x{}",op.artifact.width,op.artifact.height)] {if !input.lines().map(str::trim).any(|l|l==required){return Err(invalid("comparison full-decode framehash profile differs"));}}
    for line in input.lines().map(str::trim).filter(|l|!l.is_empty()) {
        if let Some(v)=line.strip_prefix("#tb 0:"){if tb.is_some()||!hashes.is_empty(){return Err(invalid("duplicate/out-of-order comparison timebase"));}tb=Some(crate::temporal::RationalTime::parse(v.trim()).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?);continue;}if line.starts_with('#'){continue;}
        let f=line.split(',').map(str::trim).collect::<Vec<_>>();if f.len()!=6||f[0]!="0"||hashes.len() as u64>=op.request.frame_count{return Err(invalid("comparison decoded cardinality differs"));}
        let t=tb.ok_or_else(||invalid("comparison decoded clock missing"))?;let tick=|v:&str|->Result<crate::temporal::RationalTime>{crate::temporal::RationalTime::ticks(v.parse::<i64>().map_err(|_|invalid("comparison frame tick is invalid"))?,t).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))};
        let expected=p.checked_mul(hashes.len() as u64).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;
        if tick(f[1])?!=expected||tick(f[2])?!=expected||tick(f[3])?!=p||f[4].parse::<u64>().ok()!=u64::from(op.artifact.width).checked_mul(u64::from(op.artifact.height)).and_then(|n|n.checked_mul(3)){return Err(invalid("comparison decode changed full RGB geometry or original cadence"));}
        crate::provider::require_sha256(f[5],"comparison decoded RGB")?;hashes.push(f[5].to_owned());
    }
    if hashes.len() as u64!=op.request.frame_count{return Err(invalid("comparison full decoded extent missing"));}Ok(hashes)
}
fn viewer(op:&Operation)->String {
    // All substitutions are validated numbers; source text/paths never enter JS.
    format!(r#"<!doctype html><meta charset="utf-8"><title>Deflicker comparison excerpt</title><style>body{{font:16px system-ui;background:#141414;color:#eee;margin:2rem}}main{{display:flex;gap:1rem}}img{{max-width:45vw}}button,input{{margin:.5rem}}p{{max-width:65em}}</style><h1>Before / deflicker candidate</h1><p>Silent visual excerpt. Review intentional pulses, cuts and animation holds. This preview does not approve the complete video.</p><button id="play">Play / pause</button><input aria-label="Excerpt frame" id="frame" type="range" min="0" max="{}" value="0"><output id="position"></output><main><figure><figcaption>Before</figcaption><img id="before" alt="Original excerpt frame"></figure><figure><figcaption>Candidate</figcaption><img id="after" alt="Deflicker candidate frame"></figure></main><script>let n=0,on=false,last=0;const count={},period=1000*{}/{},first={};const slider=document.querySelector('#frame');function draw(){{let name=String(n).padStart(8,'0')+'.png';for(let kind of ['before','after'])document.getElementById(kind).src=kind+'/'+name;slider.value=n;document.querySelector('#position').textContent='Original frame '+(first+n)}}slider.oninput=()=>{{n=Number(slider.value);draw()}};document.querySelector('#play').onclick=()=>{{on=!on;last=performance.now()}};function tick(t){{if(on&&t-last>=period){{let steps=Math.floor((t-last)/period);n=(n+steps)%count;last+=steps*period;draw()}}requestAnimationFrame(tick)}}draw();requestAnimationFrame(tick);</script>"#,op.request.frame_count-1,op.request.frame_count,op.artifact.frame_rate.denominator(),op.artifact.frame_rate.numerator(),op.request.first_frame)
}
pub(super) fn inspect_frames(root:&Path,op:&Operation,frames:&[Frame])->Result<()> {
    if frames.len() as u64!=op.request.frame_count{return Err(invalid("comparison frame inventory count differs"));}let names=(0..op.request.frame_count).map(|n|OsString::from(format!("{n:08}.png"))).collect::<Vec<_>>();local::entries(root,&["preview.html"],&["before","after"],false)?;
    let c=op.request.configuration.as_ref().ok_or_else(||invalid("bypassed comparison has no frames"))?;let p=period(op)?;
    for kind in ["before","after"]{inventory(&root.join(kind),&names)?;}
    let mut total=0u64;
    for (n,f) in frames.iter().enumerate() {let ordinal=op.request.first_frame+n as u64;let time=op.artifact.video_interval.start.checked_add(p.checked_mul(ordinal).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;
        if f.original_ordinal!=ordinal||f.source_time!=time{return Err(invalid("comparison original frame/time mapping differs"));}
        for (kind,pin,rgb) in [("before",(f.before_sha256.clone(),f.before_bytes),&f.before_rgb_sha256),("after",(f.after_sha256.clone(),f.after_bytes),&f.after_rgb_sha256)] {crate::provider::require_sha256(rgb,"retained decoded RGB")?;if inspect_png(&root.join(kind).join(&names[n]),op.artifact.width as u32,op.artifact.height as u32,c.tools.maximum_frame_bytes)?!=pin{return Err(invalid("comparison PNG changed"));}total=total.checked_add(pin.1).ok_or_else(||invalid("comparison output overflow"))?;}
    }
    if total>c.tools.maximum_total_frame_bytes||identity(&root.join("preview.html"),local::DOCUMENT_BYTES)?!=(local::hash(viewer(op).as_bytes()),viewer(op).len() as u64){return Err(invalid("comparison viewer/output bound differs"));}Ok(())
}
pub(crate) fn execute_invocation(inv:&crate::ProviderInvocationRequest)->Result<()> {
    inv.validate()?;let op:Operation=serde_json::from_value(serde_json::to_value(&inv.configuration.values).map_err(|_|invalid("deflicker operation encoding"))?).map_err(|_|invalid("closed deflicker operation required"))?;op.validate()?;
    if !cfg!(unix)||inv.configuration!=op.configuration()?||inv.stage_id!="compare_deflicker"||inv.inputs.len()!=1||inv.outputs.len()!=2{return Err(invalid("outside fixed deflicker native profile"));}
    let input=&inv.inputs[0];if input.port!="source"||input.artifact_id!="deflicker_source"||input.artifact_type!="video/mp4"||input.kind!=crate::ArtifactKind::File||input.artifact_role!=crate::ArtifactRole::TemporalSource||input.stream_role!=Some(crate::StreamRole::Video){return Err(invalid("deflicker source binding differs"));}
    let comparison=inv.outputs.iter().find(|a|a.port=="comparison").ok_or_else(||invalid("comparison output missing"))?;let evidence=inv.outputs.iter().find(|a|a.port=="evidence").ok_or_else(||invalid("comparison evidence missing"))?;
    if comparison.artifact_id!="comparison"||comparison.kind!=crate::ArtifactKind::Directory||comparison.artifact_type!="application/vnd.aniflow.deflicker-comparison"||comparison.artifact_role!=crate::ArtifactRole::Intermediate||comparison.stream_role!=Some(crate::StreamRole::Video)
        ||evidence.artifact_id!="comparison_report"||evidence.kind!=crate::ArtifactKind::File||evidence.artifact_type!="application/vnd.aniflow.deflicker-report+json"||evidence.artifact_role!=crate::ArtifactRole::ValidationEvidence||evidence.stream_role!=Some(crate::StreamRole::TimedMetadata)||comparison.path.parent()!=evidence.path.parent(){return Err(invalid("comparison output contract differs"));}
    let parent=comparison.path.parent().ok_or_else(||invalid("comparison parent missing"))?;ensure_canonical_directory(parent)?;require_absent(&comparison.path)?;require_absent(&evidence.path)?;
    let settings=&op.request.configuration.as_ref().ok_or_else(||invalid("bypass cannot invoke deflicker"))?.tools;let token=CancellationToken::new();let tools=crate::video_assembly::observe_components(settings)?;let renderer=std::env::current_exe().map_err(|_|invalid("cannot identify deflicker renderer"))?;
    if identity(&renderer,crate::video_assembly::MAXIMUM_TOOL_BYTES)?.0!=op.renderer_sha256.clone().unwrap()||identity(&input.path,settings.maximum_source_bytes)?!=(op.artifact.sha256.clone(),op.artifact.byte_count){return Err(invalid("comparison source/renderer changed"));}
    for output in [&comparison.path,&evidence.path]{for source in [&input.path,&settings.ffmpeg.executable,&settings.ffprobe.executable,&renderer]{if source.starts_with(output)||output.starts_with(source){return Err(invalid("comparison output overlaps source/component"));}}}
    crate::video_assembly::require_storage(parent,op.working_bytes,settings.minimum_free_bytes)?;let private=tempfile::Builder::new().prefix(".deflicker-native-").tempdir_in(parent).map_err(|_|invalid("cannot reserve private comparison work"))?;let root=private.path().canonicalize().map_err(|_|invalid("cannot resolve private comparison work"))?;
    let result:Result<()>=(||{
        let mut c=settings.clone();c.ffmpeg.executable=root.join("ffmpeg");c.ffprobe.executable=root.join("ffprobe");
        for ((pin,dst),expected) in [(&settings.ffmpeg,&c.ffmpeg.executable),(&settings.ffprobe,&c.ffprobe.executable)].into_iter().zip(&tools) {copy_pinned(&pin.executable,dst,expected,crate::video_assembly::MAXIMUM_TOOL_BYTES,&token)?;#[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(dst,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot enable private comparison tool"))?;}}
        for (id,pin) in [("ffmpeg",&c.ffmpeg),("ffprobe",&c.ffprobe)] {
            let result=run_tool_isolated_with_limits(pin,id,&[OsString::from("-version")],&token,GroupPolicy::Inherit,Some(&root),c.timeout_milliseconds,c.maximum_capture_bytes as u64).map_err(|_|invalid("comparison version preflight failed"))?;
            let text=std::str::from_utf8(&result.stdout).map_err(|_|invalid("comparison version output invalid"))?;let words=text.lines().next().unwrap_or("").split_whitespace().collect::<Vec<_>>();
            if !result.stderr.is_empty()||words.len()<3||words[0]!=id||words[1]!="version"||words[2]!=pin.version{return Err(invalid("comparison exact native version differs"));}
        }
        let source=root.join("source.mp4");copy_pinned(&input.path,&source,&(op.artifact.sha256.clone(),op.artifact.byte_count),settings.maximum_source_bytes,&token)?;
        let out=root.join("comparison");local::directory(&out)?;let mut pins=Vec::new();let mut hashes=Vec::new();
        for (kind,after) in [("before",false),("after",true)] {let dir=out.join(kind);local::directory(&dir)?;if !invoke(&c,&arguments(&op,&source,&dir,after)?,&root,&token)?.is_empty(){return Err(invalid("comparison PNG encoder returned unexpected stdout"));}
            let names=(0..op.request.frame_count).map(|n|OsString::from(format!("{n:08}.png"))).collect::<Vec<_>>();inventory(&dir,&names)?;let mut observed=Vec::new();for name in names{observed.push(inspect_png(&dir.join(name),op.artifact.width as u32,op.artifact.height as u32,settings.maximum_frame_bytes)?);}pins.push(observed);hashes.push(rgb(&dir,&op,&c,&root,&token)?);}
        local::write(&out.join("preview.html"),viewer(&op).as_bytes())?;let p=period(&op)?;let mut frames=Vec::new();
        for n in 0..op.request.frame_count as usize {let ordinal=op.request.first_frame+n as u64;let source_time=op.artifact.video_interval.start.checked_add(p.checked_mul(ordinal).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;frames.push(Frame {original_ordinal:ordinal,source_time,before_sha256:pins[0][n].0.clone(),before_bytes:pins[0][n].1,after_sha256:pins[1][n].0.clone(),after_bytes:pins[1][n].1,before_rgb_sha256:hashes[0][n].clone(),after_rgb_sha256:hashes[1][n].clone()});}
        inspect_frames(&out,&op,&frames)?;
        if identity(&input.path,settings.maximum_source_bytes)?!=(op.artifact.sha256.clone(),op.artifact.byte_count)||identity(&source,settings.maximum_source_bytes)?!=(op.artifact.sha256.clone(),op.artifact.byte_count)||crate::video_assembly::observe_components(settings)?!=tools||crate::video_assembly::observe_components(&c)?!=tools{return Err(invalid("comparison source/components changed during native work"));}
        local::entries(&root,&["ffmpeg","ffprobe","source.mp4"],&["comparison"],false)?;
        let report=Report {schema:"aniflow.deflicker-comparison-report/v1".into(),operation:op.clone(),provider_lock_sha256:inv.provider_lock_sha256.clone(),comparison_sha256:canonical_sha256(&frames)?,frames,visual_approval:"not_performed".into(),runtime_qualified:false,audio_scope:"silent_visual_excerpt; original_audio_unchanged_unassessed".into()};
        std::fs::rename(&out,&comparison.path).map_err(|_|invalid("cannot publish comparison frames"))?;local::write(&evidence.path,&local::bytes(&report)?)?;local::sync(parent)
    })();if result.is_err(){let _retained=private.keep();}result
}
