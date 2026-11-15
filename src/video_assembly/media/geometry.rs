//! Exact displayed-picture projection from complete native frame observations.
use super::{array, integer, positive, refused, text, Result, Value};
use serde::{Deserialize, Serialize};

/// Coded pixels and the explicitly admitted visible rectangle. No resize or
/// inferred offset is represented. This first cropped profile admits only
/// progressive H.264 right/bottom codec padding, with chroma-aligned edges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceVideoGeometry {
    pub coded_width: u32,
    pub coded_height: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub crop_left: u32,
    pub crop_right: u32,
    pub crop_top: u32,
    pub crop_bottom: u32,
}

impl SourceVideoGeometry {
    pub(crate) fn cropped(&self) -> bool {
        self.crop_left != 0 || self.crop_right != 0 || self.crop_top != 0 || self.crop_bottom != 0
    }
    pub(crate) fn filter(&self) -> String {
        if self.cropped() {
            format!(
                "crop=w={}:h={}:x={}:y={}:exact=1,",
                self.display_width, self.display_height, self.crop_left, self.crop_top
            )
        } else {
            String::new()
        }
    }
    fn validate(&self, pixel_format: &str, codec: &str, maximum_pixels: u64) -> Result<()> {
        let fits = |w: u32, h: u32| {
            w >= 2
                && h >= 2
                && u64::from(w)
                    .checked_mul(u64::from(h))
                    .is_some_and(|p| p <= maximum_pixels)
        };
        if !fits(self.coded_width, self.coded_height)
            || !fits(self.display_width, self.display_height)
            || self
                .coded_width
                .checked_sub(self.crop_left)
                .and_then(|v| v.checked_sub(self.crop_right))
                != Some(self.display_width)
            || self
                .coded_height
                .checked_sub(self.crop_top)
                .and_then(|v| v.checked_sub(self.crop_bottom))
                != Some(self.display_height)
        {
            return Err(refused("coded/display geometry exceeds its pixel bound or disagrees with the observed crop rectangle"));
        }
        if self.cropped() {
            if codec != "h264"
                || self.crop_left != 0
                || self.crop_top != 0
                || self.coded_width % 16 != 0
                || self.coded_height % 16 != 0
            {
                return Err(refused(
                    "cropped source profile requires H.264 right/bottom coded padding only",
                ));
            }
            let (horizontal, vertical) = match pixel_format {
                "yuv420p" => (2, 2),
                "yuv422p" => (2, 1),
                "yuv444p" | "rgb24" | "bgr24" | "bgr0" | "0rgb" | "gbrp" => (1, 1),
                _ => {
                    return Err(refused(
                        "crop chroma alignment is outside the source pixel profile",
                    ))
                }
            };
            if self.crop_left % horizontal != 0
                || self.crop_right % horizontal != 0
                || self.crop_top % vertical != 0
                || self.crop_bottom % vertical != 0
            {
                return Err(refused(
                    "observed crop rectangle is not aligned to source chroma samples",
                ));
            }
        }
        Ok(())
    }
}

fn number(value: &Value, field: &str) -> Result<u32> {
    u32::try_from(integer(value, field)?)
        .map_err(|_| refused("geometry value is negative or exceeds u32"))
}

/// Before the complete frame probes, admit displayed and (for H.264) coded
/// dimensions. A small display rectangle cannot hide an oversized coded frame.
pub(super) fn admit_inventory(video: &Value, maximum_pixels: u64) -> Result<()> {
    for (width, height) in [("width", "height")]
        .into_iter()
        .chain((text(video, "codec_name")? == "h264").then_some(("coded_width", "coded_height")))
    {
        if positive(video, width)?
            .checked_mul(positive(video, height)?)
            .is_none_or(|pixels| pixels > maximum_pixels)
        {
            return Err(refused(
                "coded/display source pixels exceed maximum_pixels before frame decoding",
            ));
        }
    }
    Ok(())
}

pub(super) fn from_documents(
    video: &Value,
    display: &Value,
    coded: &Value,
    maximum_pixels: u64,
) -> Result<SourceVideoGeometry> {
    let displays = array(display, "frames")?;
    let codes = array(coded, "frames")?;
    if displays.is_empty() || displays.len() != codes.len() {
        return Err(refused(
            "coded/display observations require the same complete nonempty frame sequence",
        ));
    }
    let codec = text(video, "codec_name")?;
    let mut geometry = None;
    for (shown, raw) in displays.iter().zip(codes) {
        let observed = SourceVideoGeometry {
            coded_width: number(raw, "width")?,
            coded_height: number(raw, "height")?,
            display_width: number(video, "width")?,
            display_height: number(video, "height")?,
            crop_left: number(raw, "crop_left")?,
            crop_right: number(raw, "crop_right")?,
            crop_top: number(raw, "crop_top")?,
            crop_bottom: number(raw, "crop_bottom")?,
        };
        observed.validate(text(video, "pix_fmt")?, codec, maximum_pixels)?;
        if codec == "h264"
            && (number(video, "coded_width")? != observed.coded_width
                || number(video, "coded_height")? != observed.coded_height)
        {
            return Err(refused(
                "uncropped decoded frame differs from declared coded dimensions",
            ));
        }
        if number(shown, "width")? != observed.display_width
            || number(shown, "height")? != observed.display_height
            || integer(raw, "interlaced_frame")? != 0
            || ["crop_left", "crop_right", "crop_top", "crop_bottom"]
                .iter()
                .any(|field| integer(shown, field).ok() != Some(0))
        {
            return Err(refused(
                "default decoded display retains unsupported crop/interlace geometry",
            ));
        }
        for field in [
            "stream_index",
            "pts",
            "key_frame",
            "pkt_dts",
            "duration",
            "pkt_duration",
            "pix_fmt",
            "sample_aspect_ratio",
            "color_range",
            "color_space",
            "color_primaries",
            "color_transfer",
            "side_data_list",
        ] {
            if shown.get(field) != raw.get(field) {
                return Err(refused("coded/display native observations differ in clock, stream or pixel/color profile"));
            }
        }
        if geometry.as_ref().is_some_and(|prior| prior != &observed) {
            return Err(refused(
                "coded/display crop geometry changes between source frames",
            ));
        }
        geometry = Some(observed);
    }
    geometry.ok_or_else(|| refused("source geometry observation is empty"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (Value, Value, Value) {
        let stream = json!({"codec_name":"h264","width":160,"height":90,"coded_width":160,"coded_height":96,"pix_fmt":"yuv420p"});
        let frame = json!({"stream_index":0,"pts":0,"duration":512,"interlaced_frame":0,"width":160,"height":90,"crop_left":0,"crop_right":0,"crop_top":0,"crop_bottom":0});
        let mut raw = frame.clone();
        raw["height"] = json!(96);
        raw["crop_bottom"] = json!(6);
        (stream, json!({"frames":[frame]}), json!({"frames":[raw]}))
    }
    #[test]
    fn explicit_bottom_padding_preserves_display_geometry() {
        let (s, d, c) = fixture();
        admit_inventory(&s, 15360).unwrap();
        let g = from_documents(&s, &d, &c, 15360).unwrap();
        assert_eq!(g.filter(), "crop=w=160:h=90:x=0:y=0:exact=1,");
    }
    #[test]
    fn coded_pixels_cannot_hide_behind_smaller_display() {
        let (s, d, c) = fixture();
        assert!(admit_inventory(&s, 14400).is_err());
        assert!(from_documents(&s, &d, &c, 14400).is_err());
    }
    #[test]
    fn inconsistent_clock_crop_alignment_or_codec_refuses() {
        let (s, d, c) = fixture();
        for (field, value) in [
            ("crop_bottom", json!(5)),
            ("crop_left", json!(2)),
            ("height", json!(98)),
            ("pts", json!(1)),
            ("interlaced_frame", json!(1)),
        ] {
            let mut changed = c.clone();
            changed["frames"][0][field] = value;
            assert!(from_documents(&s, &d, &changed, 20000).is_err());
        }
        let mut changed = s;
        changed["codec_name"] = json!("ffv1");
        assert!(from_documents(&changed, &d, &c, 20000).is_err());
    }
    #[test]
    fn nonuniform_or_incomplete_geometry_refuses() {
        let (s, mut d, mut c) = fixture();
        let second = d["frames"][0].clone();
        d["frames"].as_array_mut().unwrap().push(second);
        assert!(from_documents(&s, &d, &c, 20000).is_err());
        let mut second = c["frames"][0].clone();
        second["crop_bottom"] = json!(4);
        c["frames"].as_array_mut().unwrap().push(second);
        assert!(from_documents(&s, &d, &c, 20000).is_err());
    }
}
