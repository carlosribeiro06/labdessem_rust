pub mod error;
pub mod study;

pub use error::IoError;
pub use study::{
    StudyConfig, read_study_config, read_study_from_config, read_study_from_path,
};
