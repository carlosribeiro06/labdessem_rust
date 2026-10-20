pub mod error;
pub mod output;
mod readers;
pub mod study;

pub use error::IoError;
pub use study::{
    StudyConfig, generate_study_config_schema, read_study_config, read_study_from_path,
};
