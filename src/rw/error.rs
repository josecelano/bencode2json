//! Custom error type for both I/O and formatting strings errors.
use std::{fmt, io};
use thiserror::Error;

/// Custom error type for both I/O and formatting errors.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Formatting error: {0}")]
    Fmt(#[from] fmt::Error),
}
