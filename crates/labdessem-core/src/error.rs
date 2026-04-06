use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    Validation(String),
}

impl CoreError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl Error for CoreError {}
