//! One error type for the whole crate, so callers never have to match on
//! several unrelated error enums.

use std::path::PathBuf;

use thiserror::Error;

/// Anything NextHarness can fail with.
#[derive(Debug, Error)]
pub enum Error {
    /// A message meant for the user, already localised by the caller.
    #[error("{0}")]
    Msg(String),

    /// Filesystem failure, with the path that caused it.
    #[error("{}: {source}", path.display())]
    Io {
        /// Path that could not be read or written.
        path: PathBuf,
        /// Underlying failure.
        source: std::io::Error,
    },

    /// The catalog (built-in or a `profiles.d` override) is not usable.
    #[error("invalid catalog: {0}")]
    Catalog(String),
}

/// Shorthand used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Build a user-facing error from anything printable.
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Msg(message.into())
    }
}

/// Attach a path to an [`std::io::Error`].
pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
