//! Temporary golden-snapshot codec for the FPHA relocation.
//!
//! Stored values are compared by `f64::to_bits` against files under the repository root
//! `testdata/golden/`. Files are written only when [`BLESS_ENV`] is `1`. This module is
//! temporary and is deleted by the relocation plan's final ticket
//! (`ticket-038-remove-golden-harness.md`).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Environment variable that switches [`check_or_bless`] from comparing to writing.
pub const BLESS_ENV: &str = "LABDESSEM_BLESS_GOLDEN";

const HEADER: &str = "# labdessem golden v1";
const SOLVE_KEYS_PREFIX: &str = "# solve_keys=";
const SOLVE_PREFIX: &str = "solve/";
const UNSTABLE: &str = "CLARIFICATION REQUIRED: unstable golden capture";
const BLESS_RUNS: usize = 3;
const SIGN_BIT: u64 = 1 << 63;
const RELATIVE_TOLERANCE: f64 = 1e-12;

/// How `solve/` keys are compared; every other key is always compared by bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveCompare {
    Exact,
    Relative1e12,
}

impl SolveCompare {
    fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Relative1e12 => "relative-1e-12",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "exact" => Some(Self::Exact),
            "relative-1e-12" => Some(Self::Relative1e12),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum GoldenValue {
    F64(u64),
    Text(String),
}

/// An ordered set of named captures; `f64` values are held as their bit patterns.
#[derive(Debug, Default)]
pub struct GoldenRecord {
    entries: BTreeMap<String, GoldenValue>,
}

impl GoldenRecord {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn f64(&mut self, key: impl Into<String>, value: f64) {
        self.insert(key.into(), GoldenValue::F64(value.to_bits()));
    }

    pub fn text(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let (key, value) = (key.into(), value.into());
        assert!(
            !value.contains(['\t', '\r', '\n']),
            "golden text value for key {key:?} contains a tab, CR or LF"
        );
        self.insert(key, GoldenValue::Text(value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert(&mut self, key: String, value: GoldenValue) {
        assert!(
            !key.contains(['\t', '\r', '\n']),
            "golden key {key:?} contains a tab, CR or LF"
        );
        assert!(
            !self.entries.contains_key(&key),
            "duplicate golden key {key:?}"
        );
        self.entries.insert(key, value);
    }
}

/// A golden check or bless failure; the string is the full human-readable report.
pub struct GoldenFailure(pub String);

impl fmt::Display for GoldenFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for GoldenFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GoldenFailure {}

pub fn render(record: &GoldenRecord, solve: SolveCompare) -> String {
    let mut out = format!("{HEADER}\n{SOLVE_KEYS_PREFIX}{}\n", solve.name());
    for (key, value) in &record.entries {
        match value {
            GoldenValue::F64(bits) => out.push_str(&format!(
                "{key}\tf64\t0x{bits:016x}\t{:?}\n",
                f64::from_bits(*bits)
            )),
            GoldenValue::Text(text) => out.push_str(&format!("{key}\ttext\t{text}\n")),
        }
    }
    out
}

/// Reads a rendered record; the hex column is authoritative and the decimal column is ignored.
pub fn parse(text: &str) -> Result<(GoldenRecord, SolveCompare), GoldenFailure> {
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err(GoldenFailure(format!("line 1: expected {HEADER:?}")));
    }
    let solve = lines
        .next()
        .and_then(|line| line.strip_prefix(SOLVE_KEYS_PREFIX))
        .and_then(SolveCompare::from_name)
        .ok_or_else(|| {
            GoldenFailure(format!(
                "line 2: expected `{SOLVE_KEYS_PREFIX}exact` or `{SOLVE_KEYS_PREFIX}relative-1e-12`"
            ))
        })?;
    let mut record = GoldenRecord::new();
    for (index, line) in lines.enumerate() {
        let number = index + 3;
        let (key, value) = parse_entry(line)
            .ok_or_else(|| GoldenFailure(format!("line {number}: malformed entry {line:?}")))?;
        if record.entries.contains_key(&key) {
            return Err(GoldenFailure(format!(
                "line {number}: duplicate key {key:?}"
            )));
        }
        record.entries.insert(key, value);
    }
    Ok((record, solve))
}

fn parse_entry(line: &str) -> Option<(String, GoldenValue)> {
    let mut parts = line.splitn(4, '\t');
    let key = parts.next()?.to_owned();
    let value = match parts.next()? {
        "f64" => {
            let bits = u64::from_str_radix(parts.next()?.strip_prefix("0x")?, 16).ok()?;
            parts.next()?;
            GoldenValue::F64(bits)
        }
        "text" => {
            let text = parts.next()?.to_owned();
            if parts.next().is_some() {
                return None;
            }
            GoldenValue::Text(text)
        }
        _ => return None,
    };
    Some((key, value))
}

/// Compares by key. Text compares by equality and `f64` by bits, except `solve/` keys under
/// [`SolveCompare::Relative1e12`], which pass within 1e-12 relative.
pub fn compare(
    expected: &GoldenRecord,
    actual: &GoldenRecord,
    solve: SolveCompare,
) -> Result<(), GoldenFailure> {
    let lines = mismatches(expected, actual, solve);
    if lines.is_empty() {
        Ok(())
    } else {
        Err(GoldenFailure(format!(
            "{} golden key(s) differ:\n{}",
            lines.len(),
            lines.join("\n")
        )))
    }
}

fn mismatches(expected: &GoldenRecord, actual: &GoldenRecord, solve: SolveCompare) -> Vec<String> {
    let mut lines = Vec::new();
    for (key, want) in &expected.entries {
        match (want, actual.entries.get(key)) {
            (_, None) => lines.push(format!("{key}: missing from actual")),
            (GoldenValue::F64(exp_bits), Some(GoldenValue::F64(act_bits))) => {
                if !f64_matches(key, *exp_bits, *act_bits, solve) {
                    let exp = f64::from_bits(*exp_bits);
                    let act = f64::from_bits(*act_bits);
                    lines.push(format!(
                        "{key}: expected 0x{exp_bits:016x} ({exp:?}), actual 0x{act_bits:016x} ({act:?}), {} ulp apart",
                        ulp_distance(exp, act)
                    ));
                }
            }
            (GoldenValue::Text(exp), Some(GoldenValue::Text(act))) => {
                if exp != act {
                    lines.push(format!("{key}: expected text {exp:?}, actual {act:?}"));
                }
            }
            (_, Some(_)) => lines.push(format!("{key}: value kind differs")),
        }
    }
    lines.extend(
        actual
            .entries
            .keys()
            .filter(|key| !expected.entries.contains_key(*key))
            .map(|key| format!("{key}: extra key in actual")),
    );
    lines
}

fn f64_matches(key: &str, expected: u64, actual: u64, solve: SolveCompare) -> bool {
    if expected == actual {
        return true;
    }
    if solve != SolveCompare::Relative1e12 || !key.starts_with(SOLVE_PREFIX) {
        return false;
    }
    let (exp, act) = (f64::from_bits(expected), f64::from_bits(actual));
    exp.is_finite()
        && act.is_finite()
        && (exp - act).abs() <= RELATIVE_TOLERANCE * exp.abs().max(act.abs())
}

fn ordered(value: f64) -> i128 {
    let bits = value.to_bits();
    let magnitude = i128::from(bits & !SIGN_BIT);
    if bits & SIGN_BIT == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Distance in representable floats between `a` and `b`; `+0.0` and `-0.0` are 0 apart.
pub fn ulp_distance(a: f64, b: f64) -> u64 {
    (ordered(a) - ordered(b)).unsigned_abs() as u64
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

pub fn bless_requested() -> bool {
    std::env::var(BLESS_ENV).is_ok_and(|value| value == "1")
}

pub fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden")
        .join(name)
}

/// Compares `capture()` with `testdata/golden/<name>`, or writes that file when blessing.
pub fn check_or_bless(name: &str, capture: impl Fn() -> GoldenRecord) -> Result<(), GoldenFailure> {
    check_or_bless_at(&golden_path(name), bless_requested(), capture)
}

/// Byte-exact variant of [`check_or_bless`] for captures that are not a [`GoldenRecord`].
pub fn check_or_bless_bytes(
    name: &str,
    capture: impl Fn() -> Vec<u8>,
) -> Result<(), GoldenFailure> {
    check_or_bless_bytes_at(&golden_path(name), bless_requested(), capture)
}

fn check_or_bless_at(
    path: &Path,
    bless: bool,
    capture: impl Fn() -> GoldenRecord,
) -> Result<(), GoldenFailure> {
    if bless {
        let first = capture();
        let mut solve = SolveCompare::Exact;
        for _ in 1..BLESS_RUNS {
            let run = capture();
            if mismatches(&first, &run, SolveCompare::Exact).is_empty() {
                continue;
            }
            let unstable = mismatches(&first, &run, SolveCompare::Relative1e12);
            if !unstable.is_empty() {
                return Err(GoldenFailure(format!(
                    "{UNSTABLE} for {}:\n{}",
                    path.display(),
                    unstable.join("\n")
                )));
            }
            solve = SolveCompare::Relative1e12;
        }
        return write_golden(path, render(&first, solve).as_bytes());
    }
    let text = fs::read_to_string(path).map_err(|error| read_failure(path, &error))?;
    let (expected, solve) = parse(&text).map_err(|failure| with_path(path, &failure))?;
    compare(&expected, &capture(), solve).map_err(|failure| with_path(path, &failure))
}

fn check_or_bless_bytes_at(
    path: &Path,
    bless: bool,
    capture: impl Fn() -> Vec<u8>,
) -> Result<(), GoldenFailure> {
    if bless {
        let first = capture();
        for _ in 1..BLESS_RUNS {
            if let Some(offset) = first_difference(&first, &capture()) {
                return Err(GoldenFailure(format!(
                    "{UNSTABLE} for {}: byte captures differ at offset {offset}",
                    path.display()
                )));
            }
        }
        return write_golden(path, &first);
    }
    let expected = fs::read(path).map_err(|error| read_failure(path, &error))?;
    let actual = capture();
    match first_difference(&expected, &actual) {
        None => Ok(()),
        Some(offset) => Err(GoldenFailure(format!(
            "{}: bytes differ at offset {offset} (expected {} bytes, actual {} bytes)",
            path.display(),
            expected.len(),
            actual.len()
        ))),
    }
}

fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    if a == b {
        return None;
    }
    Some(
        a.iter()
            .zip(b)
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len())),
    )
}

fn write_golden(path: &Path, bytes: &[u8]) -> Result<(), GoldenFailure> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|error| io_failure("create directory", dir, &error))?;
    }
    fs::write(path, bytes).map_err(|error| io_failure("write", path, &error))
}

fn read_failure(path: &Path, error: &io::Error) -> GoldenFailure {
    if error.kind() == io::ErrorKind::NotFound {
        GoldenFailure(format!(
            "golden file {} is missing; create it with `{BLESS_ENV}=1 cargo test`",
            path.display()
        ))
    } else {
        io_failure("read", path, error)
    }
}

fn io_failure(action: &str, path: &Path, error: &io::Error) -> GoldenFailure {
    GoldenFailure(format!("cannot {action} {}: {error}", path.display()))
}

fn with_path(path: &Path, failure: &GoldenFailure) -> GoldenFailure {
    GoldenFailure(format!("{}: {}", path.display(), failure.0))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        BLESS_ENV, GoldenRecord, SolveCompare, check_or_bless_at, check_or_bless_bytes_at, compare,
        fnv1a64, golden_path, parse, render, ulp_distance,
    };
    use std::cell::Cell;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("labdessem-golden-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("nested").join("case.golden")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn single(key: &str, value: f64) -> GoldenRecord {
        let mut record = GoldenRecord::new();
        record.f64(key, value);
        record
    }

    fn alternating(key: &'static str, other: f64) -> impl Fn() -> GoldenRecord {
        let calls = Cell::new(0u32);
        move || {
            let n = calls.get();
            calls.set(n + 1);
            let mut record = GoldenRecord::new();
            record.f64(key, if n.is_multiple_of(2) { 1.0 } else { other });
            record
        }
    }

    fn second_line(path: &Path) -> String {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .nth(1)
            .unwrap()
            .to_owned()
    }

    #[test]
    fn render_parse_round_trip_preserves_bits() {
        let nan = f64::from_bits(0x7ff8_dead_beef_0001);
        let mut record = GoldenRecord::new();
        record.f64("a/normal", 0.1 + 0.2);
        record.f64("a/subnormal", f64::from_bits(1));
        record.f64("a/neg_zero", -0.0);
        record.f64("a/max", f64::MAX);
        record.f64("a/nan", nan);
        record.text("b/text", "two words, no tab");
        record.text("b/empty", "");
        let rendered = render(&record, SolveCompare::Relative1e12);

        let (parsed, solve) = parse(&rendered).unwrap();

        assert_eq!(solve, SolveCompare::Relative1e12);
        assert_eq!(parsed.len(), 7);
        compare(&record, &parsed, SolveCompare::Exact).unwrap();
        assert_eq!(render(&parsed, SolveCompare::Relative1e12), rendered);
        assert!(rendered.contains("a/nan\tf64\t0x7ff8deadbeef0001\t"));
        assert!(rendered.contains("a/neg_zero\tf64\t0x8000000000000000\t-0.0\n"));
    }

    #[test]
    fn render_writes_header_then_keys_in_order() {
        let mut record = GoldenRecord::new();
        record.text("z", "last");
        record.f64("a", 1.0);

        assert_eq!(
            render(&record, SolveCompare::Exact),
            "# labdessem golden v1\n# solve_keys=exact\na\tf64\t0x3ff0000000000000\t1.0\nz\ttext\tlast\n"
        );
    }

    #[test]
    fn parse_reads_hex_column_and_ignores_decimal_column() {
        let text =
            "# labdessem golden v1\n# solve_keys=exact\nx\tf64\t0x3ff0000000000000\tnot-a-number\n";

        let (record, _) = parse(text).unwrap();

        let mut expected = GoldenRecord::new();
        expected.f64("x", 1.0);
        compare(&expected, &record, SolveCompare::Exact).unwrap();
    }

    #[test]
    fn parse_rejects_malformed_files() {
        let head = "# labdessem golden v1\n# solve_keys=exact\n";
        for text in [
            String::new(),
            "# other header\n# solve_keys=exact\n".to_owned(),
            "# labdessem golden v1\n# solve_keys=fuzzy\n".to_owned(),
            format!("{head}x\tf64\t3ff0000000000000\t1.0\n"),
            format!("{head}x\tf64\t0x3ff0000000000000\n"),
            format!("{head}x\tint\t1\n"),
            format!("{head}x\ttext\ta\tb\n"),
            format!("{head}\n"),
            format!("{head}x\ttext\ta\nx\ttext\tb\n"),
        ] {
            assert!(parse(&text).is_err(), "accepted {text:?}");
        }
    }

    #[test]
    fn fnv1a64_matches_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn ulp_distance_counts_adjacent_floats() {
        let next = f64::from_bits(1.0f64.to_bits() + 1);
        let tiny = f64::from_bits(1);

        assert_eq!(ulp_distance(1.0, 1.0), 0);
        assert_eq!(ulp_distance(1.0, next), 1);
        assert_eq!(ulp_distance(next, 1.0), 1);
        assert_eq!(ulp_distance(0.0, -0.0), 0);
        assert_eq!(ulp_distance(-0.0, tiny), 1);
        assert_eq!(ulp_distance(-tiny, tiny), 2);
    }

    #[test]
    fn compare_reports_missing_extra_and_text_mismatch() {
        let mut expected = GoldenRecord::new();
        expected.f64("gone", 1.0);
        expected.text("label", "old");
        let mut actual = GoldenRecord::new();
        actual.text("label", "new");
        actual.f64("added", 2.0);

        let report = compare(&expected, &actual, SolveCompare::Exact)
            .unwrap_err()
            .0;

        assert!(report.starts_with("3 golden key(s) differ"), "{report}");
        assert!(report.contains("gone: missing from actual"), "{report}");
        assert!(report.contains("added: extra key in actual"), "{report}");
        assert!(
            report.contains("label: expected text \"old\", actual \"new\""),
            "{report}"
        );
    }

    #[test]
    fn compare_exact_mismatch_reports_both_patterns_and_ulp_distance() {
        let mut expected = GoldenRecord::new();
        expected.f64("v", 1.0);
        let mut actual = GoldenRecord::new();
        actual.f64("v", f64::from_bits(1.0f64.to_bits() + 3));

        let report = compare(&expected, &actual, SolveCompare::Exact)
            .unwrap_err()
            .0;

        assert!(report.contains("0x3ff0000000000000 (1.0)"), "{report}");
        assert!(report.contains("0x3ff0000000000003"), "{report}");
        assert!(report.contains("3 ulp apart"), "{report}");
    }

    #[test]
    fn compare_applies_relative_tolerance_to_solve_keys_only() {
        let within = 1.0 + 1e-15;
        let beyond = 1.0 + 1e-9;
        for (value, mode, passes) in [
            (within, SolveCompare::Exact, false),
            (within, SolveCompare::Relative1e12, true),
            (beyond, SolveCompare::Exact, false),
            (beyond, SolveCompare::Relative1e12, false),
        ] {
            let result = compare(
                &single("solve/objective", 1.0),
                &single("solve/objective", value),
                mode,
            );
            assert_eq!(result.is_ok(), passes, "{value} in {mode:?}");
        }

        let mut expected = GoldenRecord::new();
        expected.f64("calc/value", 1.0);
        let mut actual = GoldenRecord::new();
        actual.f64("calc/value", within);
        assert!(compare(&expected, &actual, SolveCompare::Relative1e12).is_err());
    }

    #[test]
    fn compare_never_applies_relative_tolerance_to_non_finite_values() {
        let mut expected = GoldenRecord::new();
        expected.f64("solve/objective", f64::INFINITY);
        let mut actual = GoldenRecord::new();
        actual.f64("solve/objective", 1.0);

        assert!(compare(&expected, &actual, SolveCompare::Relative1e12).is_err());
    }

    #[test]
    fn bless_writes_exact_mode_for_stable_capture() {
        let scratch = Scratch::new("stable");

        check_or_bless_at(&scratch.file(), true, || single("solve/objective", 2.5)).unwrap();

        assert_eq!(second_line(&scratch.file()), "# solve_keys=exact");
        check_or_bless_at(&scratch.file(), false, || single("solve/objective", 2.5)).unwrap();
        assert!(
            check_or_bless_at(&scratch.file(), false, || single(
                "solve/objective",
                2.5 + 1e-15
            ))
            .is_err()
        );
    }

    #[test]
    fn bless_precheck_rejects_unstable_exact_key() {
        let scratch = Scratch::new("unstable");

        let error = check_or_bless_at(
            &scratch.file(),
            true,
            alternating("calc/value", 1.0 + 1e-15),
        )
        .unwrap_err()
        .0;

        assert!(
            error.starts_with("CLARIFICATION REQUIRED: unstable golden capture"),
            "{error}"
        );
        assert!(error.contains("calc/value"), "{error}");
        assert!(!scratch.file().exists());
    }

    #[test]
    fn bless_precheck_selects_relative_mode_for_jittery_solve_key() {
        let scratch = Scratch::new("jitter");

        check_or_bless_at(
            &scratch.file(),
            true,
            alternating("solve/objective", 1.0 + 1e-15),
        )
        .unwrap();

        assert_eq!(second_line(&scratch.file()), "# solve_keys=relative-1e-12");
        check_or_bless_at(&scratch.file(), false, || {
            single("solve/objective", 1.0 + 1e-15)
        })
        .unwrap();
    }

    #[test]
    fn bless_precheck_rejects_solve_key_beyond_tolerance() {
        let scratch = Scratch::new("beyond");

        let error = check_or_bless_at(
            &scratch.file(),
            true,
            alternating("solve/objective", 1.0 + 1e-9),
        )
        .unwrap_err()
        .0;

        assert!(
            error.starts_with("CLARIFICATION REQUIRED: unstable golden capture"),
            "{error}"
        );
        assert!(error.contains("solve/objective"), "{error}");
        assert!(!scratch.file().exists());
    }

    #[test]
    fn bless_bytes_writes_stable_capture_and_rejects_unstable_one() {
        let scratch = Scratch::new("bytes");
        let calls = Cell::new(0u8);

        check_or_bless_bytes_at(&scratch.file(), true, || b"abc".to_vec()).unwrap();
        assert_eq!(fs::read(scratch.file()).unwrap(), b"abc");
        fs::remove_file(scratch.file()).unwrap();

        let error = check_or_bless_bytes_at(&scratch.file(), true, || {
            calls.set(calls.get() + 1);
            vec![b'a', calls.get()]
        })
        .unwrap_err()
        .0;

        assert!(
            error.starts_with("CLARIFICATION REQUIRED: unstable golden capture"),
            "{error}"
        );
        assert!(error.contains("offset 1"), "{error}");
        assert!(!scratch.file().exists());
    }

    #[test]
    fn check_bytes_reports_first_differing_offset() {
        let scratch = Scratch::new("bytes-check");
        check_or_bless_bytes_at(&scratch.file(), true, || b"abcdef".to_vec()).unwrap();

        check_or_bless_bytes_at(&scratch.file(), false, || b"abcdef".to_vec()).unwrap();
        let error = check_or_bless_bytes_at(&scratch.file(), false, || b"abXdef".to_vec())
            .unwrap_err()
            .0;
        assert!(error.contains("offset 2"), "{error}");
        let error = check_or_bless_bytes_at(&scratch.file(), false, || b"abc".to_vec())
            .unwrap_err()
            .0;
        assert!(error.contains("offset 3"), "{error}");
    }

    #[test]
    fn check_missing_file_names_path_and_bless_command() {
        let scratch = Scratch::new("missing");

        let error = check_or_bless_at(&scratch.file(), false, GoldenRecord::new)
            .unwrap_err()
            .0;

        assert!(error.contains("case.golden"), "{error}");
        assert!(error.contains(&format!("{BLESS_ENV}=1")), "{error}");
    }

    #[test]
    fn golden_path_resolves_to_repository_testdata() {
        let path = golden_path("case.golden");

        assert!(path.starts_with(env!("CARGO_MANIFEST_DIR")));
        assert!(path.ends_with("../../testdata/golden/case.golden"));
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../Cargo.toml")
                .exists()
        );
    }

    #[test]
    #[should_panic(expected = "duplicate golden key")]
    fn duplicate_key_is_rejected() {
        let mut record = GoldenRecord::new();
        record.f64("k", 1.0);
        record.text("k", "again");
    }

    #[test]
    #[should_panic(expected = "contains a tab, CR or LF")]
    fn tab_in_text_value_is_rejected() {
        GoldenRecord::new().text("k", "a\tb");
    }

    #[test]
    #[should_panic(expected = "contains a tab, CR or LF")]
    fn newline_in_key_is_rejected() {
        GoldenRecord::new().f64("a\nb", 1.0);
    }
}
