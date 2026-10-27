//! Fixed bounded-chunk composition; the ordinary v3 runtime owns acceptance.
use super::*;

impl UpscaylChunkOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        let size = self.settings.execution.frames_per_chunk().ok_or_else(|| invalid("chunk operations require bounded_chunk execution"))? as usize;
        if self.schema != UPSCAYL_CHUNK_OPERATION_SCHEMA_V1 || self.chunk_ordinal >= UPSCAYL_MAXIMUM_CHUNKS as u64
            || self.frames.is_empty() || self.frames.len() > size {
            return Err(invalid("invalid bounded Upscayl chunk operation"));
        }
        let first = &self.frames[0];
        let mut names = BTreeSet::new();
        for (offset, frame) in self.frames.iter().enumerate() {
            validate_frame_spec(&self.settings, frame, UPSCAYL_MAXIMUM_CHUNK_FRAMES)?;
            if frame.ordinal != self.chunk_ordinal * size as u64 + offset as u64
                || !names.insert(&frame.filename)
                || frame.source_sha256 != first.source_sha256 || frame.source_byte_count != first.source_byte_count
                || frame.frame_index_sha256 != first.frame_index_sha256
                || frame.input_width != first.input_width || frame.input_height != first.input_height {
                return Err(invalid("chunk frames must be one exact contiguous source range with unique names and shared geometry"));
            }
        }
        Ok(())
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        configuration(self, UPSCAYL_CHUNK_PROVIDER_ID, UPSCAYL_CHUNK_CAPABILITY_ID, chunk_schema_reference())
    }
    pub fn manifest(&self) -> Result<crate::ProviderManifest> { self.validate()?; manifest(&self.settings, false) }
}

impl UpscaylChunkCollectOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        let size = self.settings.execution.frames_per_chunk().ok_or_else(|| invalid("chunk collection requires bounded_chunk execution"))? as u64;
        require_sha256(&self.frame_index_sha256, "original chunk frame index")?;
        if self.schema != UPSCAYL_CHUNK_COLLECT_OPERATION_SCHEMA_V1
            || !(1..=UPSCAYL_MAXIMUM_CHUNK_FRAMES as u64).contains(&self.frame_count)
            || self.frame_count.div_ceil(size) > UPSCAYL_MAXIMUM_CHUNKS as u64 {
            return Err(invalid("chunk collection exceeds 100000 frames or 1024 chunk checkpoints"));
        }
        Ok(())
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        configuration(self, UPSCAYL_CHUNK_COLLECT_PROVIDER_ID, UPSCAYL_CHUNK_COLLECT_CAPABILITY_ID, chunk_collect_schema_reference())
    }
    pub fn manifest(&self) -> Result<crate::ProviderManifest> { self.validate()?; manifest(&self.settings, true) }
}

pub fn chunk_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:UPSCAYL_CHUNK_OPERATION_SCHEMA_V1.to_owned(), version:"1.0.0".to_owned(), sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/upscayl-recovery/chunk-operation.schema.json"))) }
}
pub fn chunk_collect_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:UPSCAYL_CHUNK_COLLECT_OPERATION_SCHEMA_V1.to_owned(), version:"1.0.0".to_owned(), sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/upscayl-recovery/chunk-collect-operation.schema.json"))) }
}

fn manifest(settings: &UpscaylRecoveryConfiguration, collect: bool) -> Result<crate::ProviderManifest> {
    let (provider, capability, schema) = if collect { (UPSCAYL_CHUNK_COLLECT_PROVIDER_ID,UPSCAYL_CHUNK_COLLECT_CAPABILITY_ID,chunk_collect_schema_reference()) }
        else { (UPSCAYL_CHUNK_PROVIDER_ID,UPSCAYL_CHUNK_CAPABILITY_ID,chunk_schema_reference()) };
    let chunk_type = "application/vnd.aniflow.upscayl-chunk-frames";
    let report_type = "application/vnd.aniflow.upscayl-chunk-report+json";
    let (inputs, outputs) = if collect {
        (vec![port("index","application/vnd.aniflow.rife-frame-index+json","validation_evidence","timed_metadata","one"), port("frames",chunk_type,"intermediate","video","one_or_more"),port("evidence",report_type,"validation_evidence","timed_metadata","one_or_more")],
         vec![port("frames","application/vnd.aniflow.upscayl-frames","intermediate","video","one"),port("index","application/vnd.aniflow.upscayl-output-index+json","validation_evidence","timed_metadata","one"),port("evidence","application/vnd.aniflow.upscayl-chunk-recovery-report+json","validation_evidence","timed_metadata","one")])
    } else {
        (vec![port("frames","image/png","intermediate","video","one_or_more")],vec![port("frames",chunk_type,"intermediate","video","one"),port("evidence",report_type,"validation_evidence","timed_metadata","one")])
    };
    let tools: Vec<_> = [("upscayl-bin",&settings.upscayl),("ffmpeg",&settings.import.ffmpeg)].into_iter()
        .map(|(id,pin)| json!({"id":id,"version_requirement":format!("={}",pin.version),"sha256":pin.sha256})).collect();
    let models = vec![json!({"id":"upscayl-param","version_requirement":"=1.0.0","sha256":settings.model.param.sha256}),json!({"id":"upscayl-weights","version_requirement":"=1.0.0","sha256":settings.model.weights.sha256})];
    let value = json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":provider,"version":"1.0.0","display_name":"aniflow bounded Upscayl chunks"},"configuration_schemas":[schema.clone()],"capabilities":[{
        "id":capability,"version":"1.0.0","kind":"frame_processor","configuration_schema":schema,"inputs":inputs,"outputs":outputs,"batching":"whole_artifact","max_concurrency":1,
        "requirements":{"tools":tools,"codecs":[],"models":models,"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":if collect {"forbidden"} else {"optional"},"network":"forbidden"}},
        "behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":if collect {vec!["filesystem_read","filesystem_write"]} else {vec!["filesystem_read","filesystem_write","environment_read","subprocess","gpu"]}},
        "lifecycle":{"progress":"none","cancellation":"process_signal"}}],
        "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
    crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
}

/// Same-run topology is checked at generic run/resume entry points too. A report
/// supplied as a loose source artifact can never masquerade as accepted history.
pub(super) fn preflight_plan(plan: &crate::PipelineV3Plan, registry: &crate::ProviderRegistry, cancellation: &CancellationToken) -> Result<()> {
    let inputs: BTreeMap<_,_> = plan.payload.inputs.iter().map(|input| (input.id.as_str(),input)).collect();
    let source_video = inputs.get("source_video");
    let mut checked = BTreeSet::new();
    let mut chunks = BTreeMap::new();
    let mut collectors = Vec::new();
    for stage in &plan.payload.stages {
        let id = stage.capability.id.as_str();
        if !matches!(id, UPSCAYL_CHUNK_CAPABILITY_ID | UPSCAYL_CHUNK_COLLECT_CAPABILITY_ID) { continue; }
        check_cancelled(cancellation)?;
        if stage.capability.version != "1.0.0" { return Err(invalid("unsupported Upscayl chunk capability version")); }
        let registration = registry.registration(&stage.provider_lock.payload.registration_id).ok_or_else(|| invalid("Upscayl chunk registration is missing"))?;
        let config = registration.configuration();
        if config.effective_configuration_sha256 != stage.provider_lock.payload.effective_configuration_sha256 || config.capability.id != id {
            return Err(invalid("Upscayl chunk settings differ from the locked plan"));
        }
        let value = serde_json::to_value(&config.values).map_err(|error| invalid(error.to_string()))?;
        let settings = if id == UPSCAYL_CHUNK_CAPABILITY_ID {
            let operation: UpscaylChunkOperation = serde_json::from_value(value).map_err(|_| invalid("invalid Upscayl chunk operation"))?;
            if operation.provider_configuration()? != *config { return Err(invalid("Upscayl chunk operation differs from its lock")); }
            let expected_inputs: Vec<_> = operation.frames.iter().map(|frame| format!("source_frame_{:06}",frame.ordinal)).collect();
            if stage.id != format!("upscale_chunk_{:06}",operation.chunk_ordinal)
                || !stage.depends_on.is_empty() || stage.inputs.len() != 1
                || stage.inputs[0].port != "frames" || stage.inputs[0].artifacts != expected_inputs
                || stage.outputs.len() != 2
                || !stage.outputs.iter().any(|output| output.port == "frames" && output.artifacts.len() == 1 && output.artifacts[0].id == format!("chunk_frames_{:06}",operation.chunk_ordinal))
                || !stage.outputs.iter().any(|output| output.port == "evidence" && output.artifacts.len() == 1 && output.artifacts[0].id == format!("chunk_evidence_{:06}",operation.chunk_ordinal)) {
                return Err(invalid("Upscayl chunk requires its fixed independent source-bound topology"));
            }
            for frame in &operation.frames {
                let input_id = format!("source_frame_{:06}",frame.ordinal);
                if inputs.get(input_id.as_str()).is_none_or(|input| input.content_sha256 != frame.input_sha256 || input.byte_count != frame.input_byte_count)
                    || source_video.is_none_or(|input| input.content_sha256 != frame.source_sha256 || input.byte_count != frame.source_byte_count) {
                    return Err(invalid("Upscayl chunk source binding differs from its exact frame identity"));
                }
            }
            chunks.insert(stage.id.clone(),operation.clone());
            operation.settings
        } else {
            let operation: UpscaylChunkCollectOperation = serde_json::from_value(value).map_err(|_| invalid("invalid Upscayl chunk collector operation"))?;
            if operation.provider_configuration()? != *config { return Err(invalid("Upscayl chunk collector differs from its lock")); }
            collectors.push((stage,operation.clone()));
            operation.settings
        };
        if checked.insert(canonical_sha256(&settings)?) { verify_components(&settings,cancellation)?; }
    }
    if chunks.len() > UPSCAYL_MAXIMUM_CHUNKS || collectors.len() > 1 { return Err(invalid("Upscayl chunk topology exceeds its fixed bound")); }
    for (collector,operation) in collectors {
        let size = operation.settings.execution.frames_per_chunk().ok_or_else(|| invalid("missing chunk execution policy"))? as u64;
        let count = operation.frame_count.div_ceil(size);
        let stages: Vec<_> = (0..count).map(|ordinal| format!("upscale_chunk_{ordinal:06}")).collect();
        let frames: Vec<_> = (0..count).map(|ordinal| format!("chunk_frames_{ordinal:06}")).collect();
        let reports: Vec<_> = (0..count).map(|ordinal| format!("chunk_evidence_{ordinal:06}")).collect();
        let has_binding = |port: &str, ids: &[String]| collector.inputs.iter().filter(|binding| binding.port == port && binding.artifacts == ids).count() == 1;
        if collector.id != "collect_upscaled_chunks" || collector.depends_on != stages || chunks.len() != count as usize
            || collector.inputs.len() != 3 || !has_binding("index", &["source_index".to_owned()])
            || !has_binding("frames", &frames) || !has_binding("evidence", &reports)
            || !inputs.contains_key("source_index") {
            return Err(invalid("Upscayl chunk collector requires every exact same-run accepted chunk producer; loose outputs cannot be adopted"));
        }
        for ordinal in 0..count {
            let chunk = chunks.get(&stages[ordinal as usize]).ok_or_else(|| invalid("Upscayl chunk producer is missing"))?;
            let expected_length = (operation.frame_count - ordinal * size).min(size);
            if chunk.settings != operation.settings || chunk.frames.len() as u64 != expected_length
                || chunk.frames.iter().any(|frame| frame.frame_index_sha256 != operation.frame_index_sha256) {
                return Err(invalid("Upscayl chunk producer differs from the complete locked source range"));
            }
        }
    }
    Ok(())
}

pub(super) fn prepare(request: &UpscaylRecoveryRequest, cancellation: &CancellationToken) -> Result<Prepared> {
    let settings = &request.configuration;
    let size = settings.execution.frames_per_chunk().ok_or_else(|| invalid("bounded_chunk execution is required"))? as usize;
    let (index,_) = frame_import::read_index(&request.frame_index)?;
    let frame_count = index.frames.len();
    let count = frame_count.div_ceil(size);
    if frame_count > UPSCAYL_MAXIMUM_CHUNK_FRAMES || count > UPSCAYL_MAXIMUM_CHUNKS {
        return Err(invalid("bounded chunk recovery admits at most 100000 frames and 1024 chunks; increase the explicit chunk size or choose a smaller input"));
    }
    let admitted = frame_import::prepare_inputs(&import_request(request),cancellation)?;
    if admitted.index != index { return Err(invalid("source frame index changed during chunk admission")); }
    settings.output_dimensions(index.width,index.height)?;
    let component_bytes = verify_components(settings,cancellation)?;
    // Deliberately conservative allowance for append-only stage manifests,
    // source bindings, operation records, directory evidence and checkpoints.
    // The manifests repeat stage records, not every source binding.
    let metadata_bound = (count as u64 + 1).checked_mul(frame_count as u64 + count as u64 + 1)
        .and_then(|value| value.checked_mul(16_384)).ok_or_else(|| invalid("chunk metadata budget overflow"))?;
    let required_free_bytes = (frame_count as u64).checked_mul(settings.maximum_output_frame_bytes)
        .and_then(|value| value.checked_mul(3)).and_then(|value| value.checked_add(metadata_bound))
        .and_then(|value| value.checked_add(component_bytes * 2))
        .and_then(|value| value.checked_add(size as u64 * (settings.import.maximum_frame_bytes + settings.maximum_output_frame_bytes)))
        .and_then(|value| value.checked_add(4 * UPSCAYL_MAXIMUM_REPORT_BYTES))
        .ok_or_else(|| invalid("Upscayl chunk working-data bound overflow"))?;
    let working = required_free_bytes.checked_add(admitted.source.1).and_then(|value| value.checked_add(admitted.frame_bytes));
    if working.is_none_or(|value| value > settings.maximum_working_bytes) {
        return Err(invalid("Upscayl chunk candidates, accepted copies, metadata and scratch exceed maximum_working_bytes; choose an explicit smaller PNG bound or larger storage budget"));
    }
    let collector_bytes = (frame_count as u64).checked_mul(settings.maximum_output_frame_bytes)
        .and_then(|value| value.checked_add(2 * UPSCAYL_MAXIMUM_REPORT_BYTES)).ok_or_else(|| invalid("collector artifact budget overflow"))?;
    if request.execution_limits.maximum_artifact_files < frame_count as u64 + 2
        || request.execution_limits.maximum_artifact_bytes < collector_bytes {
        return Err(invalid("provider artifact limits cannot hold the declared complete chunk collection"));
    }
    let index_digest = canonical_sha256(&admitted.index)?;
    let timeline = admitted.index.temporal_inspection.video_timeline().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    let mut registry = crate::ProviderRegistry::new();
    let mut bindings = vec![crate::PipelineInputBinding::new("source_video",&request.source_video),crate::PipelineInputBinding::new("source_index",&request.frame_index)];
    let mut inputs = vec![json!({"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}),json!({"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"})];
    let mut stages = Vec::with_capacity(count + 1);
    let mut frame_ids = Vec::with_capacity(count);
    let mut report_ids = Vec::with_capacity(count);
    let mut stage_ids = Vec::with_capacity(count);
    let mut frame_weights = BTreeMap::new();
    for (chunk_ordinal,chunk) in admitted.index.frames.chunks(size).enumerate() {
        let stage_id = format!("upscale_chunk_{chunk_ordinal:06}");
        let frame_id = format!("chunk_frames_{chunk_ordinal:06}");
        let evidence_id = format!("chunk_evidence_{chunk_ordinal:06}");
        let mut specs = Vec::with_capacity(chunk.len());
        let mut input_ids = Vec::with_capacity(chunk.len());
        for frame in chunk {
            check_cancelled(cancellation)?;
            let input_id = format!("source_frame_{:06}",frame.ordinal);
            let path = request.frames_directory.join(&frame.filename);
            let observed = identity(&path,settings.import.maximum_frame_bytes)?;
            if observed.0 != frame.sha256 { return Err(invalid("source frame changed during Upscayl chunk planning")); }
            specs.push(UpscaylFrameSpec {
                source_sha256:admitted.source.0.clone(),source_byte_count:admitted.source.1,frame_index_sha256:index_digest.clone(),ordinal:frame.ordinal,filename:frame.filename.clone(),source_time:timeline.frames[frame.ordinal as usize].source_time,input_sha256:observed.0,input_byte_count:observed.1,input_width:admitted.index.width,input_height:admitted.index.height,
            });
            inputs.push(json!({"id":input_id,"artifact_type":"image/png","artifact_role":"intermediate","stream_role":"video"}));
            bindings.push(crate::PipelineInputBinding::new(&input_id,path));
            input_ids.push(input_id);
        }
        let operation = UpscaylChunkOperation { schema:UPSCAYL_CHUNK_OPERATION_SCHEMA_V1.to_owned(),settings:settings.clone(),chunk_ordinal:chunk_ordinal as u64,frames:specs };
        registry.register(crate::ProviderRegistration::new(&stage_id,operation.manifest()?,operation.provider_configuration()?,request.provider_executable.clone(),"aniflow-upscayl-chunk-v1",inventory(settings))?)?;
        stages.push(json!({"id":stage_id,"depends_on":[],"capability":{"id":UPSCAYL_CHUNK_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":stage_id},"fallbacks":[]},"inputs":[{"port":"frames","artifacts":input_ids}],
            "outputs":[{"port":"frames","artifacts":[{"id":frame_id,"relative_path":format!("artifacts/upscayl-chunks/{chunk_ordinal:06}/frames"),"kind":"directory"}]},{"port":"evidence","artifacts":[{"id":evidence_id,"relative_path":format!("artifacts/upscayl-chunks/{chunk_ordinal:06}/chunk.json"),"kind":"file"}]}],
            "validations":[{"id":format!("{frame_id}_integrity"),"artifact":frame_id,"contract":"aniflow.validation/artifact-integrity/v1"},{"id":format!("{evidence_id}_integrity"),"artifact":evidence_id,"contract":"aniflow.validation/artifact-integrity/v1"}]}));
        frame_weights.insert(stage_id.clone(),chunk.len() as u64);
        frame_ids.push(frame_id); report_ids.push(evidence_id); stage_ids.push(stage_id);
    }
    let collect = UpscaylChunkCollectOperation { schema:UPSCAYL_CHUNK_COLLECT_OPERATION_SCHEMA_V1.to_owned(),settings:settings.clone(),frame_index_sha256:index_digest,frame_count:frame_count as u64 };
    registry.register(crate::ProviderRegistration::new("collect_upscaled_chunks",collect.manifest()?,collect.provider_configuration()?,request.provider_executable.clone(),"aniflow-upscayl-chunk-collect-v1",inventory(settings))?)?;
    stages.push(json!({"id":"collect_upscaled_chunks","depends_on":stage_ids,"capability":{"id":UPSCAYL_CHUNK_COLLECT_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"collect_upscaled_chunks"},"fallbacks":[]},"inputs":[{"port":"index","artifacts":["source_index"]},{"port":"frames","artifacts":frame_ids},{"port":"evidence","artifacts":report_ids}],
        "outputs":[{"port":"frames","artifacts":[{"id":"upscaled_frames","relative_path":"artifacts/upscaled/frames","kind":"directory"}]},{"port":"index","artifacts":[{"id":"upscaled_index","relative_path":"artifacts/upscaled/index.json","kind":"file"}]},{"port":"evidence","artifacts":[{"id":"upscale_evidence","relative_path":"artifacts/upscaled/upscale.json","kind":"file"}]}],
        "validations":[{"id":"upscaled_frames_integrity","artifact":"upscaled_frames","contract":"aniflow.validation/artifact-integrity/v1"},{"id":"upscaled_index_integrity","artifact":"upscaled_index","contract":"aniflow.validation/artifact-integrity/v1"},{"id":"upscale_evidence_integrity","artifact":"upscale_evidence","contract":"aniflow.validation/artifact-integrity/v1"}]}));
    let authored = json!({"schema":"aniflow.pipeline/v3","name":"upscayl-bounded-chunk-recovery","inputs":inputs,"stages":stages,"outputs":[{"id":"upscaled_frames","artifact":"upscaled_frames","required_validations":["upscaled_frames_integrity"]},{"id":"upscaled_index","artifact":"upscaled_index","required_validations":["upscaled_index_integrity"]},{"id":"upscale_evidence","artifact":"upscale_evidence","required_validations":["upscale_evidence_integrity"]}]});
    let pipeline = crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&authored)?).map_err(|failure| Error::new(failure.category(),failure.message))?;
    let mut context = crate::audio_inspection::context();
    context.allowed_side_effects.push(crate::SideEffect::Gpu);
    let plan = crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&context).map_err(|failure| Error::new(failure.category(),failure.message))?;
    frame_import::bind_prepared_plan(&plan,&admitted)?;
    if canonical_json_bytes(&plan)?.len() as u64 > MAXIMUM_PLAN_BYTES { return Err(invalid("bounded Upscayl chunk plan exceeds 64 MiB")); }
    check_cancelled(cancellation)?;
    Ok(Prepared { plan,registry,bindings,frame_weights,required_free_bytes })
}
