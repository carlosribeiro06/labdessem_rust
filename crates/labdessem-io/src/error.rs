use std::{error::Error, fmt, path::PathBuf};

#[derive(Debug)]
pub enum IoError {
    Csv(csv::Error),
    Json(serde_json::Error),
    Core(labdessem_core::CoreError),
    MissingFileName(PathBuf),
    InvalidData(String),
}

impl IoError {
    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::InvalidData(message.into())
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csv(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::Core(error) => write!(f, "{error}"),
            Self::MissingFileName(path) => {
                write!(f, "missing file name for path {}", path.display())
            }
            Self::InvalidData(message) => write!(f, "{message}"),
        }
    }
}

impl Error for IoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Csv(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Core(error) => Some(error),
            Self::MissingFileName(_) | Self::InvalidData(_) => None,
        }
    }
}

impl From<csv::Error> for IoError {
    fn from(value: csv::Error) -> Self {
        Self::Csv(value)
    }
}

impl From<labdessem_core::CoreError> for IoError {
    fn from(value: labdessem_core::CoreError) -> Self {
        Self::Core(value)
    }
}

impl From<serde_json::Error> for IoError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
