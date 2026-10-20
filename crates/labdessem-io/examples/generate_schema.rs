//! Generate JSON Schema for study_config.json.
//!
//! Run with: cargo run -p labdessem-io --example generate_schema

// Rationale: Script that runs once; expect for file write is acceptable
#![allow(clippy::expect_used)]

use labdessem_io::generate_study_config_schema;
use std::path::Path;

fn main() {
    let schema = generate_study_config_schema();
    let output_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/study_config.schema.json");
    std::fs::write(&output_path, &schema).expect("failed to write schema");
    println!("Generated {}", output_path.display());
}
