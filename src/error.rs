use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable, automation-oriented categories for public failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorCategory {
    Input,
    Configuration,
    Dependency,
    Media,
    Execution,
    State,
    Io,
    Internal,
}

impl ErrorCategory {
    /// Stable CLI exit code associated with this category.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Input => 3,
            Self::Configuration => 4,
            Self::Dependency => 5,
            Self::Media => 6,
            Self::Execution => 7,
            Self::State => 8,
            Self::Io => 9,
            Self::Internal => 70,
        }
    }
}

/// A typed public error with a stable category and actionable message.
#[derive(Debug)]
pub struct Error {
    category: ErrorCategory,
    message: String,
    source: Option<anyhow::Error>,
    temporal: Option<crate::temporal::TemporalDiagnostic>,
    validation: Option<crate::validation::ValidationDiagnostic>,
    cache: Option<crate::cache_v3::CacheDiagnostic>,
}

impl Error {
    #[must_use]
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            source: None,
            temporal: None,
            validation: None,
            cache: None,
        }
    }

    pub(crate) fn from_anyhow(category: ErrorCategory, source: anyhow::Error) -> Self {
        let cache = source.downcast_ref::<Self>().and_then(Self::cache_diagnostic).cloned();
        let validation = source.downcast_ref::<Self>().and_then(Self::validation_diagnostic).cloned();
        let temporal = source
            .downcast_ref::<crate::temporal::TemporalDiagnostic>()
            .cloned()
            .or_else(|| {
                source
                    .downcast_ref::<Self>()
                    .and_then(Self::temporal_diagnostic)
                    .cloned()
            });
        Self {
            category,
            message: format!("{source:#}"),
            source: Some(source),
            temporal,
            validation,
            cache,
        }
    }

    pub(crate) fn from_validation(diagnostic: crate::validation::ValidationDiagnostic) -> Self {
        Self { category: ErrorCategory::State, message: diagnostic.message.clone(), source: None, temporal: None, validation: Some(diagnostic), cache: None }
    }

    pub(crate) fn from_cache(diagnostic: crate::cache_v3::CacheDiagnostic) -> Self {
        Self { category: ErrorCategory::State, message: diagnostic.message.clone(), source: None, temporal: None, validation: None, cache: Some(diagnostic) }
    }

    #[must_use]
    pub fn cache_diagnostic(&self) -> Option<&crate::cache_v3::CacheDiagnostic> { self.cache.as_ref() }

    #[must_use]
    pub fn validation_diagnostic(&self) -> Option<&crate::validation::ValidationDiagnostic> {
        self.validation.as_ref()
    }

    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        self.category
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn temporal_diagnostic(&self) -> Option<&crate::temporal::TemporalDiagnostic> {
        self.temporal.as_ref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|source| source.as_ref())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
