//! Bounded MP4 box/header and UTF-8 mdta inspection, separate from FFprobe.
//! Structural leaf payloads, sample entries and coded signal are out of scope.
use super::{MetadataInventory, invalid};
use crate::{CancellationToken, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
const MAX_BOXES: usize = 8192;
const MAX_METADATA: u64 = 1_048_576;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mp4MetadataObservation {
    pub schema: String,
    pub source_sha256: String,
    pub box_count: u64,
    pub mdta: BTreeMap<String, String>,
    pub unsupported_boxes: u64,
    pub inspected_scope: String,
    pub excluded_scope: String,
}
struct BoxAt {
    kind: [u8; 4],
    payload: u64,
    end: u64,
}
fn boxes(
    file: &mut std::fs::File,
    start: u64,
    end: u64,
    count: &mut usize,
    token: &CancellationToken,
) -> Result<Vec<BoxAt>> {
    let mut result = Vec::new();
    let mut offset = start;
    while offset < end {
        if token.is_cancelled() {
            return Err(invalid("MP4 metadata inspection cancelled"));
        }
        if end - offset < 8 || *count >= MAX_BOXES {
            return Err(invalid(
                "MP4 box boundary/count exceeds the bounded profile",
            ));
        }
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| invalid("cannot seek MP4 metadata"))?;
        let mut header = [0; 8];
        file.read_exact(&mut header)
            .map_err(|_| invalid("truncated MP4 box"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap());
        let mut size = u64::from(length);
        let mut head = 8;
        if length == 1 {
            let mut wide = [0; 8];
            file.read_exact(&mut wide)
                .map_err(|_| invalid("truncated extended MP4 box"))?;
            size = u64::from_be_bytes(wide);
            head = 16;
        }
        if size < head || size > end - offset {
            return Err(invalid("open-ended, truncated or overflowing MP4 box"));
        }
        *count += 1;
        result.push(BoxAt {
            kind: header[4..].try_into().unwrap(),
            payload: offset + head,
            end: offset + size,
        });
        offset += size;
    }
    Ok(result)
}
fn bytes(file: &mut std::fs::File, b: &BoxAt, budget: &mut u64) -> Result<Vec<u8>> {
    let n = b.end - b.payload;
    *budget = budget
        .checked_add(n)
        .filter(|v| *v <= MAX_METADATA)
        .ok_or_else(|| invalid("MP4 descriptive metadata exceeds 1 MiB"))?;
    file.seek(SeekFrom::Start(b.payload))
        .map_err(|_| invalid("cannot seek metadata payload"))?;
    let mut data = vec![0; n as usize];
    file.read_exact(&mut data)
        .map_err(|_| invalid("cannot read bounded metadata payload"))?;
    Ok(data)
}
fn uint(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        data.get(offset..offset + 4)
            .ok_or_else(|| invalid("truncated mdta integer"))?
            .try_into()
            .unwrap(),
    ))
}
fn mdta(
    file: &mut std::fs::File,
    meta: &BoxAt,
    count: &mut usize,
    budget: &mut u64,
    token: &CancellationToken,
) -> Result<BTreeMap<String, String>> {
    file.seek(SeekFrom::Start(meta.payload))
        .map_err(|_| invalid("cannot seek meta header"))?;
    let mut full = [0; 4];
    file.read_exact(&mut full)
        .map_err(|_| invalid("meta full-box header is missing"))?;
    if full != [0; 4] {
        return Err(invalid("unsupported MP4 meta full-box version/flags"));
    }
    let children = boxes(file, meta.payload + 4, meta.end, count, token)?;
    if children.len() != 3 {
        return Err(invalid("mdta meta requires exactly handler, keys and ilst"));
    }
    let one = |kind: [u8; 4]| -> Result<&BoxAt> {
        let found = children
            .iter()
            .filter(|b| b.kind == kind)
            .collect::<Vec<_>>();
        if found.len() != 1 {
            return Err(invalid("ambiguous or missing mdta child"));
        }
        Ok(found[0])
    };
    let handler = bytes(file, one(*b"hdlr")?, budget)?;
    if handler.get(..8) != Some(&[0u8; 8][..]) || handler.get(8..12) != Some(&b"mdta"[..]) {
        return Err(invalid("metadata handler is not the selected mdta profile"));
    }
    let keys = bytes(file, one(*b"keys")?, budget)?;
    if uint(&keys, 0)? != 0 {
        return Err(invalid("unsupported mdta keys flags"));
    }
    let n = uint(&keys, 4)? as usize;
    if n > 256 {
        return Err(invalid("too many mdta keys"));
    }
    let mut names = Vec::new();
    let mut offset = 8;
    for _ in 0..n {
        let size = uint(&keys, offset)? as usize;
        if size < 9
            || size > 264
            || offset.checked_add(size).is_none_or(|end| end > keys.len())
            || keys.get(offset + 4..offset + 8) != Some(&b"mdta"[..])
        {
            return Err(invalid("unsupported mdta key namespace/length"));
        }
        let name = std::str::from_utf8(&keys[offset + 8..offset + size])
            .map_err(|_| invalid("mdta key is not UTF-8"))?
            .to_owned();
        if name.chars().any(char::is_control) || names.contains(&name) {
            return Err(invalid("duplicate or control-bearing mdta key"));
        }
        names.push(name);
        offset += size;
    }
    if offset != keys.len() {
        return Err(invalid("mdta keys contain trailing bytes"));
    }
    let ilst = one(*b"ilst")?;
    let items = boxes(file, ilst.payload, ilst.end, count, token)?;
    if items.len() != names.len() {
        return Err(invalid("mdta keys/items disagree"));
    }
    let mut result = BTreeMap::new();
    for item in items {
        let index = u32::from_be_bytes(item.kind) as usize;
        let name = names
            .get(
                index
                    .checked_sub(1)
                    .ok_or_else(|| invalid("mdta item index is zero"))?,
            )
            .ok_or_else(|| invalid("mdta item has no key"))?;
        let data = boxes(file, item.payload, item.end, count, token)?;
        if data.len() != 1 || data[0].kind != *b"data" {
            return Err(invalid("mdta item is not one UTF-8 data box"));
        }
        let data = bytes(file, &data[0], budget)?;
        if uint(&data, 0)? != 1 || uint(&data, 4)? != 0 || data.len() > 8200 {
            return Err(invalid("mdta item has unsupported type, locale or length"));
        }
        let text = std::str::from_utf8(&data[8..])
            .map_err(|_| invalid("mdta value is not UTF-8"))?
            .to_owned();
        if result.insert(name.clone(), text).is_some() {
            return Err(invalid("duplicate mdta item"));
        }
    }
    Ok(result)
}
pub fn inspect(
    path: &Path,
    maximum: u64,
    token: &CancellationToken,
) -> Result<Mp4MetadataObservation> {
    let identity = crate::frame_io::identity(path, maximum)?;
    let mut file = crate::audio_inspection::process::open_regular(path)
        .map_err(|_| invalid("cannot open MP4 inspection subject"))?;
    let mut count = 0;
    let mut budget = 0;
    let mut result = None;
    let mut unsupported = 0;
    let top = boxes(&mut file, 0, identity.1, &mut count, token)?;
    if top.iter().filter(|b| b.kind == *b"moov").count() != 1
        || !top.iter().any(|b| b.kind == *b"ftyp")
        || !top.iter().any(|b| b.kind == *b"mdat")
    {
        return Err(invalid(
            "MP4 inspection requires ftyp, one moov and media data",
        ));
    }
    let mut stack = top.into_iter().map(|b| (b, 0u8)).collect::<Vec<_>>();
    while let Some((b, depth)) = stack.pop() {
        if depth > 12 || token.is_cancelled() {
            return Err(invalid("MP4 metadata depth/cancellation bound reached"));
        }
        match &b.kind {
            b"meta" => {
                if result.is_some() {
                    return Err(invalid(
                        "multiple metadata containers are outside the selected scope",
                    ));
                }
                result = Some(mdta(&mut file, &b, &mut count, &mut budget, token)?);
            }
            b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" | b"edts" | b"dinf" | b"udta" => {
                stack.extend(
                    boxes(&mut file, b.payload, b.end, &mut count, token)?
                        .into_iter()
                        .map(|c| (c, depth + 1)),
                );
            }
            b"ftyp" | b"mdat" | b"mvhd" | b"tkhd" | b"mdhd" | b"hdlr" | b"vmhd" | b"smhd"
            | b"nmhd" | b"dref" | b"stsd" | b"stts" | b"ctts" | b"stsc" | b"stsz" | b"stco"
            | b"co64" | b"stss" | b"sdtp" | b"sgpd" | b"sbgp" | b"elst" => {}
            b"free" | b"skip" => {
                if bytes(&mut file, &b, &mut budget)?.iter().any(|v| *v != 0) {
                    unsupported += 1;
                }
            }
            _ => unsupported += 1,
        }
    }
    if crate::frame_io::identity(path, maximum)? != identity {
        return Err(invalid("MP4 bytes changed during metadata inspection"));
    }
    Ok(Mp4MetadataObservation {schema:"aniflow.mp4-mdta-observation/v1".into(),source_sha256:identity.0,box_count:count as u64,
        mdta:result.unwrap_or_default(),unsupported_boxes:unsupported,
        inspected_scope:"bounded MP4 box headers, zero padding and UTF-8 global mdta keys/items".into(),
        excluded_scope:"structural leaf payloads, codec sample entries, coded media/SEI, signatures and signal marks".into()})
}
pub fn agrees(observed: &Mp4MetadataObservation, inventory: &MetadataInventory) -> Result<()> {
    if observed.unsupported_boxes != 0
        || !inventory.chapters.is_empty()
        || !inventory.programs.is_empty()
    {
        return Err(invalid("unsupported final MP4 metadata/container scope"));
    }
    let global = inventory
        .global
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "major_brand" | "minor_version" | "compatible_brands"
            )
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<BTreeMap<_, _>>();
    if global != observed.mdta {
        return Err(invalid("independent mdta fields disagree with FFprobe"));
    }
    Ok(())
}
