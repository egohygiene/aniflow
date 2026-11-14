use super::*;
fn layout(root: &Path, intent: &WithRifeIntent) -> Result<()> {
    ensure_canonical_directory(root)?;
    ensure_canonical_directory(&root.join("children"))?;
    let mut files = vec!["recovered-rife-intent.json"];
    let mut dirs = vec!["children"];
    if present(&root.join("carrier"), true)? {
        dirs.push("carrier");
    }
    if present(&root.join("rife-assembly.json"), false)? {
        if intent.request.interpolation.is_none() {
            return Err(state("unselected recovered RIFE assembly marker"));
        }
        files.push("rife-assembly.json");
    }
    local::entries(root, &files, &dirs, true)?;
    let mut names = intent
        .spatial
        .batches
        .iter()
        .enumerate()
        .map(|(i, _)| format!("missing_{i:04}"))
        .collect::<Vec<_>>();
    names.push("assembly".into());
    if intent.request.interpolation.is_some() {
        names.push("interpolate".into());
    }
    let names = names
        .into_iter()
        .filter_map(|id| match present(&child(root, &id), true) {
            Ok(true) => Some(Ok(id)),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>>>()?;
    local::entries(
        &root.join("children"),
        &[],
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
        false,
    )
}
fn retained(root: &Path, token: &CancellationToken) -> Result<WithRifeIntent> {
    ensure_canonical_directory(root)?;
    let intent: WithRifeIntent =
        workspace::read_document(&root.join("recovered-rife-intent.json"))?;
    if intent.schema != "aniflow.recovered-music-video-intent/v2"
        || intent.intent_sha256 != digest(&intent)?
    {
        return Err(state("recovered v2 immutable intent changed"));
    }
    layout(root, &intent)?;
    if plan(&intent.request, token)?.intent != intent {
        return Err(state(
            "recovered v2 source/index, accepted origins, missing inputs or selected native/RIFE/assembly components changed",
        ));
    }
    Ok(intent)
}
pub(super) fn native(
    root: &Path,
    id: String,
    phase: WithRifePhase,
    ordinals: Vec<u64>,
    expected: Option<&PipelineV3Plan>,
) -> Result<WithRifeStep> {
    let value =
        super::super::runtime::native(root, id, RecoveredPhase::Assembly, ordinals, expected)?;
    Ok(WithRifeStep {
        id: value.id,
        phase,
        state: value.state,
        ordinals: value.ordinals,
        run_directory: value.run_directory,
        native_plan_sha256: value.native_plan_sha256,
    })
}
pub fn status(root: &Path, token: &CancellationToken) -> Result<WithRifeStatus> {
    crate::destination::cancelled(token)?;
    let intent = retained(root, token)?;
    let mut steps = Vec::new();
    let mut ready = true;
    for (i, part) in intent.spatial.batches.iter().enumerate() {
        let expected = if ready {
            Some(selected::plan(
                &selected_request(&intent.request.base, part.clone())?,
                token,
            )?)
        } else {
            None
        };
        let step = native(
            root,
            format!("missing_{i:04}"),
            WithRifePhase::MissingFrames,
            part.clone(),
            expected.as_ref(),
        )?;
        ready &= step.state == RecoveredStepState::Complete;
        steps.push(step);
    }
    let directory = root.join("carrier");
    let mut carrier_sha256 = None;
    if present(&directory, true)? {
        if !ready {
            return Err(state(
                "recovered complete carrier exists before accepted missing work",
            ));
        }
        let pin = identity(&directory.join("carrier.json"), MAXIMUM_DOCUMENT_BYTES)?;
        let carrier = carrier::observe(&directory, &pin.0, token)?;
        if carrier.selection.request != completed_request(root, &intent.spatial) {
            return Err(state(
                "recovered v2 carrier differs from exact selected original/new native children",
            ));
        }
        carrier_sha256 = Some(pin.0);
    }
    let mut step = pending("carrier", WithRifePhase::CompleteCarrier);
    if carrier_sha256.is_some() {
        step.state = RecoveredStepState::Complete;
        step.run_directory = Some(directory);
    }
    steps.push(step);
    ready &= carrier_sha256.is_some();
    if intent.request.interpolation.is_some() {
        let expected = if ready {
            Some(rife::recovered::runtime::plan(
                &rife_request(root, &intent, carrier_sha256.as_ref().unwrap())?,
                token,
            )?)
        } else {
            None
        };
        let step = native(
            root,
            "interpolate".into(),
            WithRifePhase::Interpolation,
            vec![],
            expected.as_ref(),
        )?;
        ready &= step.state == RecoveredStepState::Complete;
        steps.push(step);
        let marker = root.join("rife-assembly.json");
        let mut step = pending("rife-assembly", WithRifePhase::AssemblyCarrier);
        if present(&marker, false)? {
            if !ready {
                return Err(state(
                    "recovered RIFE assembly carrier exists before actual native acceptance",
                ));
            }
            let request = rife_request(root, &intent, carrier_sha256.as_ref().unwrap())?;
            let expected = video_assembly::rife::observe_recovered_run(
                &child(root, "interpolate"),
                request.configuration,
                &request.source,
            )?;
            let saved: video_assembly::rife::RifeAssemblyIndex = workspace::read_document(&marker)?;
            if saved != expected {
                return Err(state(
                    "recovered RIFE assembly marker differs from exact current predecessor",
                ));
            }
            step.state = RecoveredStepState::Complete;
        }
        ready &= step.state == RecoveredStepState::Complete;
        steps.push(step);
    }
    let expected = if ready {
        Some(video_assembly::plan(
            &assembly_request(root, &intent)?,
            token,
        )?)
    } else {
        None
    };
    let step = native(
        root,
        "assembly".into(),
        WithRifePhase::Assembly,
        vec![],
        expected.as_ref(),
    )?;
    let complete = ready && step.state == RecoveredStepState::Complete;
    steps.push(step);
    let completion = if complete {
        Some(crate::final_video::observe_assembly(child(
            root, "assembly",
        ))?)
    } else {
        None
    };
    if retained(root, token)? != intent {
        return Err(state("recovered v2 authority changed during status"));
    }
    Ok(WithRifeStatus {schema:"aniflow.recovered-music-video-status/v2".into(),run_directory:root.to_path_buf(),intent_sha256:intent.intent_sha256,reused_frames:intent.spatial.initial_selection.frames.len() as u64,missing_frames:intent.spatial.initial_selection.missing.len() as u64,
        interpolation_selected:intent.request.interpolation.is_some(),steps,carrier_sha256,complete,completion,creative_approval:"not_performed; native interpolation and import declarations do not approve the complete movie".into(),publication_authority:"none".into()})
}
fn disjoint(root: &Path, intent: &WithRifeIntent) -> Result<()> {
    let base = &intent.request.base;
    let mut paths = vec![&base.spatial.source_video, &base.spatial.source_index];
    paths.extend(intent.spatial.components.iter().map(|p| &p.path));
    paths.extend(intent.interpolation_components.iter().map(|p| &p.path));
    paths.extend(
        intent
            .spatial
            .initial_selection
            .origins
            .iter()
            .map(|o| &o.run_directory),
    );
    if let Some(path) = &base.frames_directory {
        paths.push(path);
    }
    for input in paths {
        if root.starts_with(input) || input.starts_with(root) {
            return Err(state(
                "recovered v2 root overlaps original evidence, source or components",
            ));
        }
    }
    Ok(())
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    super::super::runtime::write(path, bytes)
}
pub fn run<F: FnMut(WithRifePhase, &PipelineV3RunProgress)>(
    request: WithRifeRequest,
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<WithRifeStatus> {
    let p = plan(&request, token)?;
    let parent = local::parent(root)?;
    disjoint(root, &p.intent)?;
    video_assembly::require_storage(
        parent,
        p.intent.projected_working_bytes,
        request.base.minimum_free_bytes,
    )?;
    local::directory(root)?;
    local::directory(&root.join("children"))?;
    write(&root.join("recovered-rife-intent.json"), &bytes(&p.intent)?)?;
    drive(root, token, progress)
}
pub fn resume<F: FnMut(WithRifePhase, &PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<WithRifeStatus> {
    retained(root, token)?;
    drive(root, token, progress)
}
fn drive<F: FnMut(WithRifePhase, &PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    mut progress: F,
) -> Result<WithRifeStatus> {
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let intent = retained(root, token)?;
    disjoint(root, &intent)?;
    let mut locked = std::collections::BTreeSet::new();
    let mut guards = Vec::new();
    for origin in &intent.spatial.initial_selection.origins {
        if locked.insert(origin.run_directory.clone()) {
            guards.push(crate::cache_v3::ExclusiveWriter::acquire(
                &origin.run_directory,
                "writer.lock",
            )?);
        }
    }
    let initial = status(root, token)?;
    if initial.complete {
        return Ok(initial);
    }
    let base = &intent.request.base;
    video_assembly::admit_source(
        &base.assembly,
        &base.spatial.source_video,
        &intent
            .spatial
            .initial_selection
            .source_index
            .temporal_inspection,
        root,
        token,
    )?;
    for _ in 0..intent.spatial.batches.len() + 5 {
        crate::destination::cancelled(token)?;
        let current = status(root, token)?;
        for step in &current.steps {
            if step.phase == WithRifePhase::MissingFrames
                && step.state == RecoveredStepState::Complete
            {
                let path = step.run_directory.as_ref().unwrap();
                if locked.insert(path.clone()) {
                    guards.push(crate::cache_v3::ExclusiveWriter::acquire(
                        path,
                        "writer.lock",
                    )?);
                }
            }
        }
        let current = status(root, token)?;
        if current.complete {
            return Ok(current);
        }
        let next = current
            .steps
            .iter()
            .find(|s| s.state != RecoveredStepState::Complete)
            .ok_or_else(|| state("recovered v2 no next phase"))?;
        let phase = next.phase;
        match phase {
            WithRifePhase::MissingFrames => {
                let request = selected_request(base, next.ordinals.clone())?;
                let path = child(root, &next.id);
                let expected = selected::plan(&request, token)?;
                if next.native_plan_sha256.as_deref() != Some(expected.plan_sha256.as_str()) {
                    return Err(state(
                        "recovered v2 next native plan changed after current status",
                    ));
                }
                let report = |e: &PipelineV3RunProgress| progress(phase, e);
                if present(&path, true)? {
                    selected::resume(request, &path, token, report)?;
                } else {
                    selected::run_at(request, &path, &expected, token, report)?;
                }
            }
            WithRifePhase::CompleteCarrier => {
                carrier::create(
                    &completed_request(root, &intent.spatial),
                    &root.join("carrier"),
                    token,
                )?;
            }
            WithRifePhase::Interpolation => {
                let request =
                    rife_request(root, &intent, current.carrier_sha256.as_ref().unwrap())?;
                let path = child(root, "interpolate");
                let expected = rife::recovered::runtime::plan(&request, token)?;
                if next.native_plan_sha256.as_deref() != Some(expected.plan_sha256.as_str()) {
                    return Err(state(
                        "recovered v2 next native plan changed after current status",
                    ));
                }
                let report = |e: &PipelineV3RunProgress| progress(phase, e);
                if present(&path, true)? {
                    rife::recovered::runtime::resume(request, &path, token, report)?;
                } else {
                    rife::recovered::runtime::run_at(request, &path, &expected, token, report)?;
                }
            }
            WithRifePhase::AssemblyCarrier => {
                let request =
                    rife_request(root, &intent, current.carrier_sha256.as_ref().unwrap())?;
                video_assembly::rife::bind_recovered_run(
                    &child(root, "interpolate"),
                    request.configuration,
                    &request.source,
                    &root.join("rife-assembly.json"),
                )?;
            }
            WithRifePhase::Assembly => {
                let request = assembly_request(root, &intent)?;
                let path = child(root, "assembly");
                let expected = video_assembly::plan(&request, token)?;
                if next.native_plan_sha256.as_deref() != Some(expected.plan_sha256.as_str()) {
                    return Err(state(
                        "recovered v2 next native plan changed after current status",
                    ));
                }
                let report = |e: &PipelineV3RunProgress| progress(phase, e);
                if present(&path, true)? {
                    video_assembly::resume(request, &path, token, report)?;
                } else {
                    video_assembly::run_at(request, &path, &expected, token, report)?;
                }
            }
        }
    }
    Err(state("recovered v2 exceeded fixed finite phase count"))
}
pub fn completion(
    root: &Path,
    token: &CancellationToken,
) -> Result<crate::final_video::FinalVideoHandoff> {
    let before = status(root, token)?;
    let result = before
        .completion
        .clone()
        .ok_or_else(|| state("recovered v2 has no currently accepted complete assembly"))?;
    if status(root, token)? != before {
        return Err(state("recovered v2 changed during final handoff"));
    }
    Ok(result)
}
