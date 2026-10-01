use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

use super::{TemporalCode, diagnostic};

/// Exact signed seconds or an exact dimensionless rate, with a positive denominator.
/// Arithmetic fails rather than saturating or rounding. JSON must be reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RationalTime {
    pub(super) numerator: i64,
    pub(super) denominator: u64,
}

impl<'de> Deserialize<'de> for RationalTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            numerator: i64,
            denominator: u64,
        }
        let raw = Raw::deserialize(deserializer)?;
        let value = Self::new(raw.numerator, raw.denominator).map_err(de::Error::custom)?;
        if value.numerator != raw.numerator || value.denominator != raw.denominator {
            return Err(de::Error::custom("rational time must be reduced"));
        }
        Ok(value)
    }
}

impl RationalTime {
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    #[must_use]
    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    pub fn new(numerator: i64, denominator: u64) -> anyhow::Result<Self> {
        if denominator == 0 || denominator > i64::MAX as u64 {
            return Err(diagnostic(
                TemporalCode::InvalidRational,
                None,
                "denominator must be in 1..=i64::MAX",
            )
            .into());
        }
        Self::from_wide(i128::from(numerator), i128::from(denominator))
    }

    fn from_wide(numerator: i128, denominator: i128) -> anyhow::Result<Self> {
        if denominator <= 0 {
            return Err(diagnostic(
                TemporalCode::InvalidRational,
                None,
                "denominator must be positive",
            )
            .into());
        }
        let mut a = numerator.unsigned_abs();
        let mut b = denominator as u128;
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let divisor = a.max(1) as i128;
        let n = numerator / divisor;
        let d = denominator / divisor;
        if n < i128::from(i64::MIN) || n > i128::from(i64::MAX) || d > i128::from(i64::MAX) {
            return Err(diagnostic(
                TemporalCode::ArithmeticOverflow,
                None,
                "exact rational is outside the supported integer range",
            )
            .into());
        }
        Ok(Self {
            numerator: n as i64,
            denominator: d as u64,
        })
    }

    pub fn validate(self) -> anyhow::Result<()> {
        if Self::new(self.numerator, self.denominator)? != self {
            return Err(diagnostic(
                TemporalCode::InvalidRational,
                None,
                "rational time must be reduced",
            )
            .into());
        }
        Ok(())
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let (n, d) = value.split_once('/').ok_or_else(|| {
            diagnostic(
                TemporalCode::InvalidRational,
                None,
                "expected numerator/denominator",
            )
        })?;
        let n = n.parse::<i64>().map_err(|_| {
            diagnostic(
                TemporalCode::InvalidRational,
                None,
                "invalid integer numerator",
            )
        })?;
        let d = d.parse::<u64>().map_err(|_| {
            diagnostic(
                TemporalCode::InvalidRational,
                None,
                "invalid integer denominator",
            )
        })?;
        Self::new(n, d)
    }

    /// Parse finite decimal text exactly, without a floating-point intermediate.
    pub fn decimal(value: &str) -> anyhow::Result<Self> {
        let negative = value.starts_with('-');
        let unsigned = value.strip_prefix('-').unwrap_or(value);
        let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        if whole.is_empty()
            || !whole.bytes().all(|c| c.is_ascii_digit())
            || !fraction.bytes().all(|c| c.is_ascii_digit())
            || fraction.len() > 18
        {
            return Err(diagnostic(
                TemporalCode::InvalidRational,
                None,
                "expected a finite decimal with at most 18 fractional digits",
            )
            .into());
        }
        let denominator = 10_i128.pow(fraction.len() as u32);
        let whole = whole.parse::<i128>().map_err(|_| {
            diagnostic(
                TemporalCode::ArithmeticOverflow,
                None,
                "decimal magnitude overflow",
            )
        })?;
        let fractional = if fraction.is_empty() {
            0
        } else {
            fraction.parse::<i128>().map_err(|_| {
                diagnostic(
                    TemporalCode::InvalidRational,
                    None,
                    "invalid decimal fraction",
                )
            })?
        };
        let numerator = whole
            .checked_mul(denominator)
            .and_then(|n| n.checked_add(fractional))
            .ok_or_else(|| {
                diagnostic(
                    TemporalCode::ArithmeticOverflow,
                    None,
                    "decimal magnitude overflow",
                )
            })?;
        Self::from_wide(if negative { -numerator } else { numerator }, denominator)
    }

    pub fn ticks(ticks: i64, time_base: Self) -> anyhow::Result<Self> {
        time_base.validate()?;
        Self::from_wide(
            i128::from(ticks) * i128::from(time_base.numerator),
            i128::from(time_base.denominator),
        )
    }

    pub fn checked_add(self, other: Self) -> anyhow::Result<Self> {
        self.validate()?;
        other.validate()?;
        Self::from_wide(
            i128::from(self.numerator) * i128::from(other.denominator)
                + i128::from(other.numerator) * i128::from(self.denominator),
            i128::from(self.denominator) * i128::from(other.denominator),
        )
    }

    pub fn checked_sub(self, other: Self) -> anyhow::Result<Self> {
        self.validate()?;
        other.validate()?;
        Self::from_wide(
            i128::from(self.numerator) * i128::from(other.denominator)
                - i128::from(other.numerator) * i128::from(self.denominator),
            i128::from(self.denominator) * i128::from(other.denominator),
        )
    }

    pub fn checked_mul(self, count: u64) -> anyhow::Result<Self> {
        self.validate()?;
        Self::from_wide(
            i128::from(self.numerator) * i128::from(count),
            i128::from(self.denominator),
        )
    }

    pub fn reciprocal(self) -> anyhow::Result<Self> {
        self.validate()?;
        if self.numerator <= 0 {
            return Err(diagnostic(
                TemporalCode::InvalidRational,
                None,
                "rate or period must be positive",
            )
            .into());
        }
        Self::new(self.denominator as i64, self.numerator as u64)
    }

    pub fn abs(self) -> anyhow::Result<Self> {
        self.validate()?;
        Self::from_wide(
            i128::from(self.numerator).abs(),
            i128::from(self.denominator),
        )
    }

    /// Compatibility presentation only. Never use this value for timing policy.
    #[must_use]
    pub fn as_seconds_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    pub fn milliseconds_ceil(self) -> anyhow::Result<u64> {
        self.validate()?;
        if self.numerator < 0 {
            return Err(diagnostic(
                TemporalCode::NegativePresentationOffset,
                None,
                "negative time has no unsigned millisecond projection",
            )
            .into());
        }
        let n = i128::from(self.numerator) * 1000;
        let d = i128::from(self.denominator);
        u64::try_from((n + d - 1) / d).map_err(|_| {
            diagnostic(
                TemporalCode::ArithmeticOverflow,
                None,
                "millisecond projection overflow",
            )
            .into()
        })
    }

    /// Explicit FFmpeg duration projection: floor to microseconds, error < 1 us.
    /// Callers must validate the actual frame grid; this is not temporal truth.
    pub fn ffmpeg_microseconds(self) -> anyhow::Result<String> {
        self.validate()?;
        if self.numerator < 0 {
            return Err(diagnostic(
                TemporalCode::NegativePresentationOffset,
                None,
                "negative FFmpeg seek projection is unsupported",
            )
            .into());
        }
        let ticks = i128::from(self.numerator) * 1_000_000 / i128::from(self.denominator);
        Ok(format!("{}.{:06}", ticks / 1_000_000, ticks % 1_000_000))
    }

    /// Round an exact nonnegative source boundary upward onto a sample clock.
    pub fn sample_boundary_ceil(self, sample_rate: u32) -> anyhow::Result<u64> {
        self.validate()?;
        if self.numerator < 0 || sample_rate == 0 {
            return Err(diagnostic(
                TemporalCode::InvalidRational,
                None,
                "sample boundary requires nonnegative time and positive rate",
            )
            .into());
        }
        let n = i128::from(self.numerator) * i128::from(sample_rate);
        let d = i128::from(self.denominator);
        u64::try_from((n + d - 1) / d).map_err(|_| {
            diagnostic(
                TemporalCode::ArithmeticOverflow,
                None,
                "sample boundary overflow",
            )
            .into()
        })
    }
}

impl Ord for RationalTime {
    fn cmp(&self, other: &Self) -> Ordering {
        (i128::from(self.numerator) * i128::from(other.denominator))
            .cmp(&(i128::from(other.numerator) * i128::from(self.denominator)))
    }
}

impl PartialOrd for RationalTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for RationalTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}
