//! Existing observer/process owners, narrowed to the single accepted mov_text
//! track. Text extraction and packet identities are independent gates.
use super::*;
pub(super) fn observe(
    settings: &AssemblyConfiguration,
    path: &Path,
    index: u32,
    document: &crate::timed_text::TimedTextDocument,
    observation: &Observation,
    scratch: &Path,
    token: &CancellationToken,
) -> Result<(String, u64)> {
    let track = stream(&observation.inventory, index)?;
    if text(track, "codec_type")? != "subtitle"
        || text(track, "codec_name")? != "mov_text"
        || text(track, "codec_tag_string")? != "tx3g"
    {
        return Err(refused(
            "subtitle preservation admits only the selected native mov_text track",
        ));
    }
    let size = positive(track, "extradata_size")?;
    let configuration = sha(track, "extradata_hash")?;
    let disposition = disposition(observation, index)?;
    let (packets, count) =
        elementary_packets(path, index, observation, settings, scratch, token, true)?;
    captions::verify_text(settings, path, index, document, scratch, token)?;
    Ok((
        digest(&(
            text(track, "codec_name")?,
            text(track, "codec_tag_string")?,
            size,
            configuration,
            disposition,
            packets,
        ))?,
        count,
    ))
}
pub(super) fn disposition(observation: &Observation, index: u32) -> Result<String> {
    let values = stream(&observation.inventory, index)?
        .get("disposition")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| refused("subtitle disposition was not observed"))?;
    let mut enabled = Vec::new();
    for (name, value) in values {
        let bit = value
            .as_u64()
            .filter(|v| *v <= 1)
            .ok_or_else(|| refused("subtitle disposition is not a boolean flag"))?;
        if bit == 1 {
            if !matches!(
                name.as_str(),
                "default"
                    | "dub"
                    | "original"
                    | "comment"
                    | "lyrics"
                    | "karaoke"
                    | "forced"
                    | "hearing_impaired"
                    | "visual_impaired"
                    | "clean_effects"
                    | "attached_pic"
                    | "timed_thumbnails"
                    | "non_diegetic"
                    | "captions"
                    | "descriptions"
                    | "metadata"
                    | "dependent"
                    | "still_image"
            ) {
                return Err(refused(
                    "subtitle disposition is outside the explicit preservation profile",
                ));
            }
            enabled.push(name.as_str());
        }
    }
    enabled.sort_unstable();
    Ok(if enabled.is_empty() {
        "0".into()
    } else {
        enabled.join("+")
    })
}
