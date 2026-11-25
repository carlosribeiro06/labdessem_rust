//! Fitness test for the unsafe-code boundary of labdessem-model.
//!
//! The workspace forbids `unsafe_code`. Cargo cannot combine `[lints] workspace = true` with an
//! override, so this crate carries its own lint tables at `deny`, forbids unsafe code on every
//! module declared in `src/lib.rs` and allows it on the qhull FFI module `hull` only. The compiler
//! cannot see two escapes: the copied clippy table drifting from the workspace one, and a second
//! attribute that lowers `unsafe_code`. These text-only checks catch both.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const LINT: &str = "unsafe_code";
const LOWERING_LEVELS: [&str; 3] = ["allow", "expect", "warn"];
const HULL: &str = "hull";

type CheckResult = Result<(), String>;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_dir().join("..").join("..")
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// Key/value pairs of the TOML table `[header]`, with comments, blank lines and whitespace removed.
fn table(text: &str, header: &str) -> BTreeMap<String, String> {
    let wanted = format!("[{header}]");
    let mut inside = false;
    let mut entries = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == wanted;
            continue;
        }
        if !inside || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        if let Some((key, value)) = compact.split_once('=') {
            entries.insert(key.to_string(), value.to_string());
        }
    }
    entries
}

/// Check (i): model's lint tables against the workspace tables.
fn lint_parity(workspace: &str, model: &str) -> CheckResult {
    let ws_clippy = table(workspace, "workspace.lints.clippy");
    let model_clippy = table(model, "lints.clippy");
    if !ws_clippy.contains_key("pedantic") {
        return Err("workspace clippy table not found".to_string());
    }
    if ws_clippy != model_clippy {
        return Err(format!(
            "model [lints.clippy] differs from [workspace.lints.clippy]: {model_clippy:?} vs {ws_clippy:?}"
        ));
    }
    let mut ws_rust = table(workspace, "workspace.lints.rust");
    let mut model_rust = table(model, "lints.rust");
    if ws_rust.remove(LINT).as_deref() != Some("\"forbid\"") {
        return Err(format!("the workspace must keep {LINT} = \"forbid\""));
    }
    if model_rust.remove(LINT).as_deref() != Some("\"deny\"") {
        return Err(format!("model must set {LINT} = \"deny\""));
    }
    if ws_rust != model_rust {
        return Err(format!(
            "model [lints.rust] differs: {model_rust:?} vs {ws_rust:?}"
        ));
    }
    Ok(())
}

/// The `members` array of the root manifest.
fn members(workspace: &str) -> Vec<String> {
    let mut inside = false;
    let mut found = Vec::new();
    for line in workspace.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") && trimmed.contains('[') {
            inside = true;
        }
        if inside {
            for piece in trimmed.split('"').skip(1).step_by(2) {
                found.push(piece.to_string());
            }
            if trimmed.contains(']') {
                break;
            }
        }
    }
    found
}

/// Check (ii) for one member manifest.
fn inherits_workspace_lints(manifest: &str) -> CheckResult {
    if manifest.lines().any(|l| l.trim().starts_with("[lints.")) {
        return Err("declares its own [lints.*] table".to_string());
    }
    if table(manifest, "lints")
        .get("workspace")
        .map(String::as_str)
        != Some("true")
    {
        return Err("lacks [lints] workspace = true".to_string());
    }
    Ok(())
}

/// A top-level `mod` declaration of lib.rs with the attributes written above it.
struct ModDecl {
    visibility: String,
    name: String,
    attributes: Vec<String>,
}

fn mod_decls(lib_rs: &str) -> Vec<ModDecl> {
    let mut decls = Vec::new();
    let mut attributes = Vec::new();
    for line in lib_rs.lines() {
        if line.starts_with("#[") {
            attributes.push(line.chars().filter(|c| !c.is_whitespace()).collect());
            continue;
        }
        if line.starts_with("//") {
            continue;
        }
        let (visibility, rest) = match line.split_once("mod ") {
            Some((vis, rest)) if vis.is_empty() || vis.starts_with("pub") => (vis.trim(), rest),
            _ => {
                attributes.clear();
                continue;
            }
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        decls.push(ModDecl {
            visibility: visibility.to_string(),
            name,
            attributes: std::mem::take(&mut attributes),
        });
    }
    decls
}

/// Check (iii): forbid on every module, a single allow on `hull`.
fn module_boundary(lib_rs: &str) -> CheckResult {
    let forbid = format!("#[forbid({LINT})]");
    let allow = format!("#[allow({LINT})]");
    let decls = mod_decls(lib_rs);
    let mut allowed = Vec::new();
    for decl in &decls {
        let has_forbid = decl.attributes.contains(&forbid);
        let has_allow = decl.attributes.contains(&allow);
        match (has_forbid, has_allow) {
            (true, false) => {}
            (false, true) => allowed.push(decl.name.clone()),
            _ => {
                return Err(format!(
                    "mod {} ({}) must carry exactly one of {forbid} or {allow}",
                    decl.name, decl.visibility
                ));
            }
        }
    }
    if allowed != [HULL] {
        return Err(format!(
            "{allow} must sit on mod {HULL} only, found {allowed:?}"
        ));
    }
    Ok(())
}

/// Byte offsets of every attribute in `source` that lowers `unsafe_code` below `deny`.
fn lowering_sites(source: &str) -> Vec<usize> {
    let mut sites = Vec::new();
    let bytes = source.as_bytes();
    let mut start = 0;
    while let Some(offset) = source[start..].find('#') {
        let hash = start + offset;
        start = hash + 1;
        let mut open = hash + 1;
        if bytes.get(open) == Some(&b'!') {
            open += 1;
        }
        if bytes.get(open) != Some(&b'[') {
            continue;
        }
        let mut depth = 0usize;
        let mut close = None;
        for (i, b) in bytes.iter().enumerate().skip(open) {
            match b {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { continue };
        let body: String = source[open + 1..close]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if lowers(&body) {
            sites.push(hash);
        }
    }
    sites
}

/// True when an attribute body names `unsafe_code` inside an allow, expect or warn list.
fn lowers(body: &str) -> bool {
    let mut search = 0;
    while let Some(offset) = body[search..].find(LINT) {
        let at = search + offset;
        search = at + LINT.len();
        let mut depth = 0usize;
        for (i, c) in body[..at].char_indices().rev() {
            match c {
                ')' => depth += 1,
                '(' if depth > 0 => depth -= 1,
                '(' => {
                    let level: String = body[..i]
                        .chars()
                        .rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if LOWERING_LEVELS.contains(&level.as_str()) {
                        return true;
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    false
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

#[test]
fn lint_tables_match_workspace() -> CheckResult {
    let workspace = read(&workspace_root().join("Cargo.toml"))?;
    let model = read(&crate_dir().join("Cargo.toml"))?;
    lint_parity(&workspace, &model)
}

#[test]
fn other_members_inherit_workspace_lints() -> CheckResult {
    let workspace = read(&workspace_root().join("Cargo.toml"))?;
    let list = members(&workspace);
    if !list.iter().any(|m| m == "crates/labdessem-model") || list.len() < 2 {
        return Err(format!("unexpected members list {list:?}"));
    }
    for member in list.iter().filter(|m| *m != "crates/labdessem-model") {
        if member.contains(['*', '?', '[']) {
            return Err(format!(
                "glob member {member} is not supported by this check"
            ));
        }
        let manifest = read(&workspace_root().join(member).join("Cargo.toml"))?;
        inherits_workspace_lints(&manifest).map_err(|e| format!("{member}: {e}"))?;
    }
    Ok(())
}

#[test]
fn lib_mods_forbid_unsafe_except_hull() -> CheckResult {
    module_boundary(&read(&crate_dir().join("src/lib.rs"))?)
}

#[test]
fn no_other_unsafe_code_exception_in_model() -> CheckResult {
    let root = crate_dir();
    let mut files = vec![root.join("build.rs")];
    for dir in ["src", "tests", "examples", "benches"] {
        rust_files(&root.join(dir), &mut files)?;
    }
    let mut found = Vec::new();
    for file in &files {
        if !file.exists() {
            continue;
        }
        let count = lowering_sites(&read(file)?).len();
        if count > 0 {
            found.push((
                file.strip_prefix(&root).unwrap_or(file).to_path_buf(),
                count,
            ));
        }
    }
    if found != [(PathBuf::from("src/lib.rs"), 1)] {
        return Err(format!(
            "the only {LINT} exception must be the hull one in src/lib.rs: {found:?}"
        ));
    }
    Ok(())
}

#[test]
fn lint_parity_check_detects_drift() {
    let workspace = format!(
        "[workspace.lints.rust]\n{LINT} = \"forbid\"\n\n[workspace.lints.clippy]\npedantic = \"warn\"\npanic = \"warn\"\n"
    );
    let good = format!(
        "[lints.rust]\n{LINT} = \"deny\"\n\n[lints.clippy]\n# copy\npedantic = \"warn\"\npanic   = \"warn\"\n"
    );
    assert!(lint_parity(&workspace, &good).is_ok());
    let drifted = good.replace("panic   = \"warn\"", "panic = \"allow\"");
    assert!(lint_parity(&workspace, &drifted).is_err());
    let missing = good.replace("panic   = \"warn\"\n", "");
    assert!(lint_parity(&workspace, &missing).is_err());
    let allowed = good.replace("\"deny\"", "\"allow\"");
    assert!(lint_parity(&workspace, &allowed).is_err());
}

#[test]
fn member_check_detects_local_lint_table() {
    assert!(
        inherits_workspace_lints("[package]\nname = \"x\"\n\n[lints]\nworkspace = true\n").is_ok()
    );
    assert!(inherits_workspace_lints("[package]\nname = \"x\"\n").is_err());
    assert!(inherits_workspace_lints("[lints.rust]\nfoo = \"allow\"\n").is_err());
    assert!(
        inherits_workspace_lints("[lints]\nworkspace = true\n[lints.clippy]\nall = \"warn\"\n")
            .is_err()
    );
}

#[test]
fn mod_check_detects_missing_forbid() {
    let forbid = format!("#[forbid({LINT})]");
    let allow = format!("#[allow({LINT})]");
    let good = format!(
        "{forbid}\npub mod a;\n{allow}\npub mod {HULL};\n#[cfg(test)]\n{forbid}\nmod tests;\n"
    );
    assert!(module_boundary(&good).is_ok());
    assert!(
        module_boundary(&good.replacen(&format!("{forbid}\npub mod a;"), "pub mod a;", 1)).is_err()
    );
    assert!(
        module_boundary(&good.replace(
            &format!("{forbid}\npub mod a;"),
            &format!("{allow}\npub mod a;")
        ))
        .is_err()
    );
    assert!(module_boundary(&good.replace(&format!("{allow}\n"), "")).is_err());
}

#[test]
fn exception_scan_detects_every_lowering_form() {
    for level in LOWERING_LEVELS {
        for form in [
            format!("#[{level}({LINT})]"),
            format!("#![{level}({LINT})]"),
            format!("#[{level}( {LINT} )]"),
            format!("#[{level}(dead_code, {LINT})]"),
            format!("#[cfg_attr(test, {level}({LINT}))]"),
            format!("#![cfg_attr(not(test), {level}(\n    {LINT}\n))]"),
        ] {
            assert_eq!(
                lowering_sites(&format!("{form}\nmod x;\n")).len(),
                1,
                "{form}"
            );
        }
    }
    for kept in ["forbid", "deny"] {
        assert_eq!(
            lowering_sites(&format!("#[{kept}({LINT})]\nmod x;\n")),
            Vec::<usize>::new()
        );
    }
}
