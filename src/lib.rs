//! # whisper
//!
//! Deterministic knowledge workspace management for AI agents.
//!
//! The mechanical half of the incitaciones `whisper` skill as a binary:
//! canonical repo keys, branch slugs, worktree slots, and scope-based
//! knowledge routing. Same inputs, same outputs — no LLM improvisation.
//!
//! Built on `genesis-vibes` for the charly-vibes envelope/CLI conventions.

pub mod config;
pub mod doctor;
pub mod workspace;

/// Version of the CLI, injected from Cargo.
pub const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tool name used in envelopes and suggestions.
pub const TOOL_NAME: &str = "whisper";

/// Error type carrying an optional self-healing suggestion.
#[derive(Debug)]
pub struct WhisperError {
    pub message: String,
    pub suggestion: Option<String>,
}

impl WhisperError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl std::fmt::Display for WhisperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(s) = &self.suggestion {
            write!(f, "\n  -> {s}")?;
        }
        Ok(())
    }
}

impl std::error::Error for WhisperError {}

impl From<std::io::Error> for WhisperError {
    fn from(err: std::io::Error) -> Self {
        Self::new(format!("I/O error: {err}"))
    }
}

pub type Result<T> = std::result::Result<T, WhisperError>;
