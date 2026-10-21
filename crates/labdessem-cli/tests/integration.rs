#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
#[ignore = "requires reference file"]
fn hydro_output_matches_reference() {
    let case_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/24Barras");
    let reference_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference");

    // Run CLI
    let status = Command::new(env!("CARGO_BIN_EXE_labdessem"))
        .args(["run", case_dir.to_str().unwrap()])
        .status()
        .expect("Failed to run labdessem");

    assert!(status.success(), "labdessem run failed");

    // Compare files
    let generated = fs::read(case_dir.join("RESULTADOS/resultado_hidreletricas.csv"))
        .expect("Failed to read generated file");
    let reference = fs::read(reference_dir.join("resultado_hidreletricas.csv"))
        .expect("Failed to read reference file");

    assert_eq!(generated, reference, "Hydro output differs from reference");
}
