//! Closed spatial filters for an explicitly selected silent comparison.
//!
//! Compilation describes requested processing. It does not observe a native
//! result, establish perceptual improvement or supply a manual review.
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::destination::invalid;

pub const SCHEMA: &str = "aniflow.spatial-polish/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denoise: Option<Denoise>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deband: Option<Deband>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpen: Option<Sharpen>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Denoise {
    /// NLMeans strength in hundredths, 100..=1000 (1.00..=10.00).
    pub strength_hundredths: u16,
    /// Odd patch size, one of 3, 5 or 7 pixels, for luma and chroma.
    pub patch_odd: u8,
    /// Odd search size, 7..=15 pixels, for luma and chroma.
    pub search_odd: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deband {
    /// The same per-plane threshold in thousandths, 1..=250.
    pub threshold_thousandths: u16,
    /// Fixed sampling distance in pixels, 1..=32; not a random radius.
    pub radius: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sharpen {
    /// Positive luma amount in hundredths, 1..=200; chroma is unchanged.
    pub amount_hundredths: u16,
    pub kernel_odd: u8,
}

impl Profile {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4096 {
            return Err(invalid("spatial polish profile exceeds 4 KiB"));
        }
        let profile: Self = crate::provider::decode_json(bytes, "spatial polish profile")?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA
            || (self.denoise.is_none() && self.deband.is_none() && self.sharpen.is_none())
        {
            return Err(invalid(
                "spatial polish requires its exact schema and an explicit selected filter",
            ));
        }
        if let Some(d) = &self.denoise {
            if !(100..=1000).contains(&d.strength_hundredths)
                || !matches!(d.patch_odd, 3 | 5 | 7)
                || !(7..=15).contains(&d.search_odd)
                || d.search_odd % 2 != 1
                || d.search_odd < d.patch_odd
            {
                return Err(invalid(
                    "spatial denoise requires bounded strength and odd patch/search sizes",
                ));
            }
        }
        if let Some(d) = &self.deband {
            if !(1..=250).contains(&d.threshold_thousandths) || !(1..=32).contains(&d.radius) {
                return Err(invalid(
                    "spatial deband requires a bounded threshold and fixed distance",
                ));
            }
        }
        if let Some(s) = &self.sharpen {
            if !(1..=200).contains(&s.amount_hundredths) || !matches!(s.kernel_odd, 3 | 5 | 7) {
                return Err(invalid(
                    "spatial sharpen requires bounded positive luma strength and an odd kernel",
                ));
            }
        }
        Ok(())
    }

    /// A configuration identity, not an output digest or qualification claim.
    pub fn identity_sha256(&self) -> Result<String> {
        self.validate()?;
        crate::provider::canonical_sha256(self)
    }

    /// Compile validated numeric settings in denoise -> deband -> sharpen order.
    /// No caller-provided filter, expression, path or shell text is accepted.
    pub fn filters(&self) -> Result<Vec<String>> {
        self.validate()?;
        let mut filters = Vec::new();
        if let Some(d) = &self.denoise {
            let strength = decimal(d.strength_hundredths, 100, 2);
            filters.push(format!(
                "nlmeans=s={strength}:p={}:pc={}:r={}:rc={}",
                d.patch_odd, d.patch_odd, d.search_odd, d.search_odd
            ));
        }
        if let Some(d) = &self.deband {
            let threshold = decimal(d.threshold_thousandths, 1000, 3);
            // Negative range/direction select fixed sampling rather than random
            // per-pixel distances/directions. Native behavior still needs qualification.
            filters.push(format!(
                "deband=1thr={threshold}:2thr={threshold}:3thr={threshold}:4thr={threshold}:range=-{}:direction=-0.7853981633974483:blur=1:coupling=0",
                d.radius
            ));
        }
        if let Some(s) = &self.sharpen {
            let amount = decimal(s.amount_hundredths, 100, 2);
            filters.push(format!(
                "unsharp=luma_msize_x={}:luma_msize_y={}:luma_amount={amount}:chroma_msize_x=3:chroma_msize_y=3:chroma_amount=0.00",
                s.kernel_odd, s.kernel_odd
            ));
        }
        Ok(filters)
    }
}

fn decimal(value: u16, scale: u16, digits: usize) -> String {
    format!("{}.{:0digits$}", value / scale, value % scale)
}

#[cfg(test)]
mod tests;
