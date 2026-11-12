//! Explicit operator-selected delivery goals. No universal platform target is
//! implied, and parsing a policy does not measure, adjust or approve audio.
use serde::{Deserialize, Serialize};

use crate::destination::invalid;
use crate::Result;

pub const SCHEMA: &str = "aniflow.audio-delivery-policy/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub id: String,
    pub revision: String,
    /// Supplied musical/delivery intent; not a verified platform requirement.
    pub purpose: String,
    pub target_milli_lufs: i32,
    pub tolerance_milli_lu: u16,
    pub maximum_true_peak_milli_dbtp: i32,
}

impl Policy {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4096 {
            return Err(invalid("audio delivery policy exceeds 4 KiB"));
        }
        let policy: Self = crate::provider::decode_json(bytes, "audio delivery policy")?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<()> {
        let tolerance = i32::from(self.tolerance_milli_lu);
        if self.schema != SCHEMA
            || !crate::destination::identifier(&self.id)
            || !crate::destination::identifier(&self.revision)
            || self.purpose.trim().is_empty()
            || self.purpose.len() > 512
            || self.purpose.chars().any(char::is_control)
            || !(-70_000..=0).contains(&self.target_milli_lufs)
            || tolerance > 3000
            || self.target_milli_lufs - tolerance < -70_000
            || self.target_milli_lufs + tolerance > 0
            || !(-20_000..=0).contains(&self.maximum_true_peak_milli_dbtp)
        {
            return Err(invalid("audio policy requires an explicit bounded goal, tolerance, nonpositive peak ceiling and supplied purpose"));
        }
        Ok(())
    }

    pub fn loudness_bounds_milli_lufs(&self) -> Result<(i32, i32)> {
        self.validate()?;
        let tolerance = i32::from(self.tolerance_milli_lu);
        Ok((self.target_milli_lufs - tolerance, self.target_milli_lufs + tolerance))
    }

    /// Identity of selected settings, not a media digest or certification.
    pub fn identity_sha256(&self) -> Result<String> {
        self.validate()?;
        crate::provider::canonical_sha256(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Policy {
        Policy {schema: SCHEMA.into(), id: "operator-preview".into(), revision: "1".into(),
            purpose: "Synthetic test choice; no platform requirement asserted".into(),
            target_milli_lufs: -14_000, tolerance_milli_lu: 500,
            maximum_true_peak_milli_dbtp: -1000}
    }

    #[test]
    fn goals_are_explicit_and_each_setting_changes_the_identity() {
        let p = example();
        assert_eq!(p.loudness_bounds_milli_lufs().unwrap(), (-14_500, -13_500));
        let initial = p.identity_sha256().unwrap();
        for field in ["target", "tolerance", "peak", "purpose", "revision"] {
            let mut changed = p.clone();
            match field {
                "target" => changed.target_milli_lufs -= 100,
                "tolerance" => changed.tolerance_milli_lu += 1,
                "peak" => changed.maximum_true_peak_milli_dbtp -= 100,
                "purpose" => changed.purpose.push_str(" changed"),
                _ => changed.revision = "2".into(),
            }
            assert_ne!(initial, changed.identity_sha256().unwrap());
        }
    }

    #[test]
    fn missing_defaults_unknown_fields_and_invalid_bounds_are_rejected() {
        let p = example(); let value = serde_json::to_value(&p).unwrap();
        for field in ["target_milli_lufs", "tolerance_milli_lu", "maximum_true_peak_milli_dbtp", "purpose"] {
            let mut bad = value.clone(); bad.as_object_mut().unwrap().remove(field);
            assert!(Policy::from_json_slice(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let mut bad = value; bad["auto_normalize"] = serde_json::json!(true);
        assert!(Policy::from_json_slice(&serde_json::to_vec(&bad).unwrap()).is_err());
        for (target,tolerance,peak) in [(-70_001,0,-1000),(-70_000,1,-1000),(-100,500,-1000),(-14_000,3001,-1000),(-14_000,500,1)] {
            let mut bad = p.clone(); bad.target_milli_lufs = target;
            bad.tolerance_milli_lu = tolerance; bad.maximum_true_peak_milli_dbtp = peak;
            assert!(bad.validate().is_err());
        }
    }
}
