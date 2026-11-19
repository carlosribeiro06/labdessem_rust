//! End-to-end FPHA cases: the real `labdessem` binary is run on derived copies of
//! `examples/24Barras` (computed and precomputed FPHA). This file is the single home of every FPHA
//! end-to-end case; an integration test cannot import bin modules, so the helpers live here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use tempfile::TempDir;

const SOURCE_CASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/24Barras");

/// Header of `OPER/OPER_FPHA.csv`, spelled exactly as in the header-only official file.
const OPER_FPHA_HEADER: &str =
    "USIH;Nome;SegFPHA;Fcorrec;Rhs.mw;Varm.mw/hm3;Qtur.mw/m3s;Qlat.mw/m3s";

/// Relative objective tolerance that separates "same LP" from "different LP" in the A1 test.
const OBJECTIVE_RELATIVE_TOLERANCE: f64 = 1e-5;

/// Geometry table the computed FPHA reads, relative to the case directory.
const GEOMETRY_FILE: &str = "CAD/CAD_GEOMETRIA_UHE.csv";

/// Hydro plant table; its `Nome` column is the source of the plant name in the A2 case.
const PLANTS_FILE: &str = "CAD/CAD_UHE.csv";

/// Hydro results table, relative to the case directory.
const HYDRO_RESULTS_FILE: &str = "RESULTADOS/resultado_hidreletricas.csv";

/// Plant whose geometry rows are removed in the A2 case: FUNIL-GRANDE in `examples/24Barras`.
const SKIPPED_PLANT_CODE: u32 = 2;

/// Tailrace table the computed FPHA reads, relative to the case directory.
const TAILRACE_FILE: &str = "CAD/CAD_TAILRACE.csv";

/// Plant whose tailrace rows are removed in the A3 case: FURNAS in `examples/24Barras`.
const NO_TAILRACE_PLANT_CODE: u32 = 3;

/// Relative path to the bytes of every file below a root; a directory maps to `None`.
type TreeSnapshot = BTreeMap<PathBuf, Option<Vec<u8>>>;

/// Value written to the `Fcorrec` column of the derived `OPER_FPHA.csv`.
#[derive(Clone, Copy, Debug)]
enum Fcorrec {
    /// `1` on every row: the sensitivity control, which drops alpha and must not reproduce the
    /// computed run.
    One,
    /// The CSV `alpha` of the plant, verbatim: the documented copy, since the printed gammas are
    /// unscaled and the model applies alpha once.
    Alpha,
}

/// One row of `RESULTADOS/resultado_fpha_computed.csv`; numbers stay verbatim strings.
#[derive(Debug)]
struct ComputedPlane {
    codigo_usina: u32,
    nome_usina: String,
    plane_index: usize,
    gamma_0: String,
    gamma_v: String,
    gamma_q: String,
    gamma_s: String,
    alpha: String,
}

/// Outcome of one `labdessem run`.
#[derive(Debug)]
struct RunOutcome {
    success: bool,
    code: Option<i32>,
    stderr: String,
    objective: Option<f64>,
    elapsed_s: f64,
}

fn fast_study_config(fpha_mode: &str) -> String {
    format!(
        r#"{{
  "opcao_execucao": 2,
  "rede": 0,
  "UCT": 0,
  "UCH": 0,
  "TON_Residual": 0,
  "FPHA": "{fpha_mode}",
  "FCF": 1,
  "EngMax": 0
}}
"#
    )
}

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create a directory of the derived case");
    for entry in fs::read_dir(source).expect("list a directory of examples/24Barras") {
        let entry = entry.expect("read a directory entry of examples/24Barras");
        let target = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .expect("stat an entry of examples/24Barras");
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file of examples/24Barras");
        }
    }
}

/// Copies `examples/24Barras/CAD` and `OPER` into a fresh temp dir and writes the fast
/// configuration with `"FPHA": <fpha_mode>`. The CLI only ever runs on such a copy.
fn derived_case(fpha_mode: &str) -> TempDir {
    let dir = TempDir::new().expect("create the derived-case temp dir");
    let source = Path::new(SOURCE_CASE);
    for subdir in ["CAD", "OPER"] {
        copy_dir(&source.join(subdir), &dir.path().join(subdir));
    }
    fs::write(
        dir.path().join("study_config.json"),
        fast_study_config(fpha_mode),
    )
    .expect("write study_config.json of the derived case");
    dir
}

fn collect_tree(root: &Path, dir: &Path, snapshot: &mut TreeSnapshot) {
    for entry in fs::read_dir(dir).expect("list a directory of the snapshot root") {
        let entry = entry.expect("read a directory entry of the snapshot root");
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("snapshot entries live below the snapshot root")
            .to_path_buf();
        if entry
            .file_type()
            .expect("stat an entry of the snapshot root")
            .is_dir()
        {
            snapshot.insert(relative, None);
            collect_tree(root, &path, snapshot);
        } else {
            let bytes = fs::read(&path).expect("read a file of the snapshot root");
            snapshot.insert(relative, Some(bytes));
        }
    }
}

fn snapshot_tree(root: &Path) -> TreeSnapshot {
    let mut snapshot = TreeSnapshot::new();
    collect_tree(root, root, &mut snapshot);
    snapshot
}

fn parse_objective(stderr: &str) -> Option<f64> {
    stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix("Objective:")?.trim().parse().ok())
}

/// Runs `labdessem run <dir>`; the `Objective:` summary line is printed on stderr.
fn run_cli(dir: &Path) -> RunOutcome {
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_labdessem"))
        .arg("run")
        .arg(dir)
        .output()
        .expect("run the labdessem binary");
    let elapsed_s = started.elapsed().as_secs_f64();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    RunOutcome {
        success: output.status.success(),
        code: output.status.code(),
        objective: parse_objective(&stderr),
        stderr,
        elapsed_s,
    }
}

/// Reads the planes of `RESULTADOS/resultado_fpha_computed.csv`: lines before the header that
/// starts with `codigo_usina;` are the preamble (its column list uses `codigo_usina:`).
fn read_computed_planes(dir: &Path) -> Vec<ComputedPlane> {
    let text = fs::read_to_string(dir.join("RESULTADOS/resultado_fpha_computed.csv"))
        .expect("read RESULTADOS/resultado_fpha_computed.csv");
    assert!(
        text.lines().any(|line| line.starts_with("codigo_usina;")),
        "resultado_fpha_computed.csv has no `codigo_usina;` header line"
    );
    text.lines()
        .skip_while(|line| !line.starts_with("codigo_usina;"))
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split(';').map(str::trim).collect();
            assert!(
                fields.len() >= 8,
                "resultado_fpha_computed.csv row has fewer than 8 fields: {line}"
            );
            ComputedPlane {
                codigo_usina: fields[0]
                    .parse()
                    .expect("parse codigo_usina of resultado_fpha_computed.csv"),
                nome_usina: fields[1].to_owned(),
                plane_index: fields[2]
                    .parse()
                    .expect("parse plane_index of resultado_fpha_computed.csv"),
                gamma_0: fields[3].to_owned(),
                gamma_v: fields[4].to_owned(),
                gamma_q: fields[5].to_owned(),
                gamma_s: fields[6].to_owned(),
                alpha: fields[7].to_owned(),
            }
        })
        .collect()
}

/// Overwrites the header-only `OPER/OPER_FPHA.csv` of the derived case with one row per plane:
/// `SegFPHA = plane_index + 1` and the CSV values verbatim.
fn write_oper_fpha(dir: &Path, planes: &[ComputedPlane], fcorrec: Fcorrec) {
    let mut table = String::from(OPER_FPHA_HEADER);
    table.push('\n');
    for plane in planes {
        let ComputedPlane {
            codigo_usina,
            nome_usina,
            plane_index,
            gamma_0,
            gamma_v,
            gamma_q,
            gamma_s,
            alpha,
        } = plane;
        let segment = plane_index + 1;
        let factor = match fcorrec {
            Fcorrec::One => "1",
            Fcorrec::Alpha => alpha.as_str(),
        };
        table.push_str(&format!(
            "{codigo_usina};{nome_usina};{segment};{factor};{gamma_0};{gamma_v};{gamma_q};{gamma_s}\n"
        ));
    }
    fs::write(dir.join("OPER/OPER_FPHA.csv"), table).expect("write OPER/OPER_FPHA.csv");
}

/// Asserts a zero exit with a finite `Objective:` line and returns the objective; a failed run
/// aborts the test with its exit code and full stderr.
fn expect_objective(label: &str, outcome: &RunOutcome) -> f64 {
    assert!(
        outcome.success,
        "{label} run failed with exit code {:?}; stderr:\n{}",
        outcome.code, outcome.stderr
    );
    let objective = outcome.objective.unwrap_or(f64::NAN);
    assert!(
        objective.is_finite(),
        "{label} run printed no finite `Objective:` line; stderr:\n{}",
        outcome.stderr
    );
    objective
}

fn assert_precomputed_stderr(label: &str, outcome: &RunOutcome) {
    assert!(
        outcome.stderr.contains("FPHA=precomputed"),
        "{label} stderr lacks `FPHA=precomputed`:\n{}",
        outcome.stderr
    );
    assert!(
        !outcome.stderr.contains("Computing FPHA hyperplanes"),
        "{label} stderr shows an FPHA computation in precomputed mode:\n{}",
        outcome.stderr
    );
}

/// Runs the CLI in precomputed mode on a fresh derived case whose `OPER_FPHA.csv` is built
/// from `planes`.
fn run_precomputed(planes: &[ComputedPlane], fcorrec: Fcorrec) -> RunOutcome {
    let case = derived_case("precomputed");
    write_oper_fpha(case.path(), planes, fcorrec);
    run_cli(case.path())
}

/// Drops the data lines of `relative` (below the derived case `case`) whose first `;` field,
/// trimmed, equals `plant_code`; keeps the header. Returns the number of dropped lines.
fn remove_plant_rows(case: &Path, relative: &str, plant_code: u32) -> usize {
    assert!(
        Path::new(relative)
            .components()
            .all(|component| matches!(component, Component::Normal(_))),
        "{relative}: derived-case paths must be relative and free of `..`"
    );
    let path = case.join(relative);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("{relative}: {error}"))
        .expect("read the CSV of the derived case");
    let mut kept = String::with_capacity(text.len());
    let mut removed = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let first_field = line.split(';').next().unwrap_or_default();
        let is_plant_row = index > 0 && first_field.trim().parse::<u32>().ok() == Some(plant_code);
        if is_plant_row {
            removed += 1;
        } else {
            kept.push_str(line);
        }
    }
    fs::write(&path, kept)
        .map_err(|error| format!("{relative}: {error}"))
        .expect("write the edited CSV of the derived case");
    removed
}

/// Position of `column` in the trimmed header fields of `file`.
fn column_index(header: &[&str], column: &str, file: &str) -> usize {
    header
        .iter()
        .position(|field| *field == column)
        .ok_or_else(|| format!("{file}: column {column} not found in the header {header:?}"))
        .expect("a required CSV column is missing")
}

/// Trimmed `Nome` of the `CAD/CAD_UHE.csv` row whose `Codigo` is `plant_code`: the name the
/// results table carries (the input pads it with spaces).
fn plant_name_in_cad(case: &Path, plant_code: u32) -> String {
    let text = fs::read_to_string(case.join(PLANTS_FILE)).expect("read CAD/CAD_UHE.csv");
    let mut lines = text.lines();
    let header: Vec<&str> = lines
        .next()
        .expect("CAD/CAD_UHE.csv has a header line")
        .split(';')
        .map(str::trim)
        .collect();
    let code_column = column_index(&header, "Codigo", PLANTS_FILE);
    let name_column = column_index(&header, "Nome", PLANTS_FILE);
    let fields: Vec<&str> = lines
        .map(|line| line.split(';').map(str::trim).collect::<Vec<&str>>())
        .find(|fields| {
            fields
                .get(code_column)
                .and_then(|code| code.parse::<u32>().ok())
                == Some(plant_code)
        })
        .ok_or_else(|| format!("{PLANTS_FILE}: no row with Codigo {plant_code}"))
        .expect("the plant row is missing");
    fields
        .get(name_column)
        .copied()
        .ok_or_else(|| format!("{PLANTS_FILE}: the row of plant {plant_code} has no Nome field"))
        .expect("the plant row is shorter than the header")
        .to_owned()
}

#[test]
fn harness_derived_case_matches_official_inputs() {
    // Arrange
    let source = Path::new(SOURCE_CASE);
    let official_before = snapshot_tree(source);

    // Act
    let case = derived_case("computed");

    // Assert
    for subdir in ["CAD", "OPER"] {
        let official = snapshot_tree(&source.join(subdir));
        let copied = snapshot_tree(&case.path().join(subdir));
        assert!(
            !official.is_empty(),
            "examples/24Barras/{subdir} has no files"
        );
        assert_eq!(
            copied.keys().collect::<Vec<_>>(),
            official.keys().collect::<Vec<_>>(),
            "{subdir}: copied file list differs from examples/24Barras"
        );
        for (relative, bytes) in &official {
            assert!(
                copied.get(relative) == Some(bytes),
                "{subdir}/{}: copied bytes differ from examples/24Barras",
                relative.display()
            );
        }
    }
    let config = fs::read_to_string(case.path().join("study_config.json"))
        .expect("read study_config.json of the derived case");
    assert!(
        config.contains("\"FPHA\": \"computed\""),
        "study_config.json must select computed FPHA:\n{config}"
    );
    assert!(
        snapshot_tree(source) == official_before,
        "examples/24Barras changed while the derived case was built"
    );
}

/// Regression for audit FP-02 and MX-06: the documented OPER_FPHA copy (Fcorrec = alpha) reproduces the computed run.
#[test]
fn computed_csv_round_trip_reproduces_objective() {
    // Arrange
    let computed_case = derived_case("computed");

    // Act
    let computed = run_cli(computed_case.path());
    let o_c = expect_objective("computed", &computed);
    assert!(
        computed.stderr.contains("Computing FPHA hyperplanes"),
        "computed stderr lacks `Computing FPHA hyperplanes`:\n{}",
        computed.stderr
    );
    let planes = read_computed_planes(computed_case.path());
    for code in 0..=3 {
        assert!(
            planes.iter().any(|plane| plane.codigo_usina == code),
            "resultado_fpha_computed.csv has no row for plant code {code}"
        );
    }
    let alpha_run = run_precomputed(&planes, Fcorrec::Alpha);
    let o_alpha = expect_objective("precomputed (Fcorrec = alpha)", &alpha_run);
    assert_precomputed_stderr("precomputed (Fcorrec = alpha)", &alpha_run);
    let one_run = run_precomputed(&planes, Fcorrec::One);
    let o_one = expect_objective("precomputed (Fcorrec = 1)", &one_run);
    assert_precomputed_stderr("precomputed (Fcorrec = 1)", &one_run);

    // Assert
    let gap_alpha = (o_alpha - o_c).abs() / o_c.abs();
    let gap_one = (o_one - o_c).abs() / o_c.abs();
    eprintln!(
        "A1 computed                    O_c     = {o_c:.4}  ({:.2} s)",
        computed.elapsed_s
    );
    eprintln!(
        "A1 precomputed, Fcorrec = alpha O_alpha = {o_alpha:.4}  ({:.2} s)",
        alpha_run.elapsed_s
    );
    eprintln!(
        "A1 precomputed, Fcorrec = 1     O_one   = {o_one:.4}  ({:.2} s)",
        one_run.elapsed_s
    );
    eprintln!("A1 relative gap |O_alpha - O_c| / |O_c| = {gap_alpha:.3e}");
    eprintln!("A1 relative gap |O_one - O_c| / |O_c|   = {gap_one:.3e}");
    assert!(
        (o_alpha - o_c).abs() <= OBJECTIVE_RELATIVE_TOLERANCE * o_c.abs(),
        "Fcorrec = alpha copy must match the computed objective within \
         {OBJECTIVE_RELATIVE_TOLERANCE:e} relative: O_alpha = {o_alpha:.4}, O_c = {o_c:.4}, \
         gap = {gap_alpha:.3e}"
    );
    assert!(
        (o_one - o_c).abs() > OBJECTIVE_RELATIVE_TOLERANCE * o_c.abs(),
        "Fcorrec = 1 copy must differ from the computed objective by more than \
         {OBJECTIVE_RELATIVE_TOLERANCE:e} relative: O_one = {o_one:.4}, O_c = {o_c:.4}, \
         gap = {gap_one:.3e}"
    );
}

/// Regression for audit HI-03 (FP-01, NU-04): a plant with units and no geometry fails the run
/// with an error naming the plant and the geometry table, before any result file is written.
#[test]
fn plant_without_geometry_fails_run() {
    // Arrange
    let case = derived_case("computed");
    let plant_name = plant_name_in_cad(case.path(), SKIPPED_PLANT_CODE);
    let removed = remove_plant_rows(case.path(), GEOMETRY_FILE, SKIPPED_PLANT_CODE);
    eprintln!(
        "A2 removed {removed} row(s) of plant {SKIPPED_PLANT_CODE} ({plant_name}) from \
         {GEOMETRY_FILE}"
    );
    assert!(
        removed >= 1,
        "plant {SKIPPED_PLANT_CODE} has no rows in {GEOMETRY_FILE}: the case would not be derived"
    );

    // Act
    let outcome = run_cli(case.path());

    // Assert
    assert_eq!(
        outcome.code,
        Some(1),
        "the run must exit 1 because plant {SKIPPED_PLANT_CODE} cannot be fitted; stderr:\n{}",
        outcome.stderr
    );
    for fragment in [
        "computed FPHA could not be fitted",
        plant_name.as_str(),
        GEOMETRY_FILE,
    ] {
        assert!(
            outcome.stderr.contains(fragment),
            "stderr lacks `{fragment}`:\n{}",
            outcome.stderr
        );
    }
    assert!(
        !case.path().join(HYDRO_RESULTS_FILE).exists(),
        "{HYDRO_RESULTS_FILE} must not be written when the FPHA computation fails"
    );
}

/// Regression for audit FP-R02 (A3): a plant with units and no tailrace row fails the run with an
/// error naming the plant and the tailrace table, before any result file is written.
#[test]
fn missing_physical_data_fails_run() {
    // Arrange
    let case = derived_case("computed");
    let plant_name = plant_name_in_cad(case.path(), NO_TAILRACE_PLANT_CODE);
    let removed = remove_plant_rows(case.path(), TAILRACE_FILE, NO_TAILRACE_PLANT_CODE);
    eprintln!(
        "A3 removed {removed} row(s) of plant {NO_TAILRACE_PLANT_CODE} ({plant_name}) from \
         {TAILRACE_FILE}"
    );
    assert!(
        removed >= 1,
        "plant {NO_TAILRACE_PLANT_CODE} has no rows in {TAILRACE_FILE}: the case would not be derived"
    );

    // Act
    let outcome = run_cli(case.path());

    // Assert
    assert_eq!(
        outcome.code,
        Some(1),
        "the run must exit 1 because plant {NO_TAILRACE_PLANT_CODE} has no tailrace; stderr:\n{}",
        outcome.stderr
    );
    for fragment in [
        "computed FPHA could not be fitted",
        plant_name.as_str(),
        TAILRACE_FILE,
    ] {
        assert!(
            outcome.stderr.contains(fragment),
            "stderr lacks `{fragment}`:\n{}",
            outcome.stderr
        );
    }
    assert!(
        !case.path().join(HYDRO_RESULTS_FILE).exists(),
        "{HYDRO_RESULTS_FILE} must not be written when the FPHA computation fails"
    );
}
