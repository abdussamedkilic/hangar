use crate::model::{Platform, PlatformEntry};
use std::fmt;

pub mod epic;
pub mod steam;
pub mod ubisoft;

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderError {
    Io(String),
    Parse(String),
    Network(String),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::Io(s) => write!(f, "io error: {s}"),
            ProviderError::Parse(s) => write!(f, "parse error: {s}"),
            ProviderError::Network(s) => write!(f, "network error: {s}"),
        }
    }
}

impl std::error::Error for ProviderError {}

pub trait LibraryProvider: Send + Sync {
    fn platform(&self) -> Platform;
    fn is_available(&self) -> bool;
    fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError>;
}
