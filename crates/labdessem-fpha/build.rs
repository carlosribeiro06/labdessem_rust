//! Build script for labdessem-fpha.
//!
//! Compiles the vendored libqhull_r reentrant library and the C shim.

#![allow(clippy::expect_used, clippy::panic)]

use std::env;
use std::path::PathBuf;

const QHULL_SOURCES: [&str; 17] = [
    "geom2_r.c",
    "geom_r.c",
    "global_r.c",
    "io_r.c",
    "libqhull_r.c",
    "mem_r.c",
    "merge_r.c",
    "poly2_r.c",
    "poly_r.c",
    "qset_r.c",
    "random_r.c",
    "stat_r.c",
    "user_r.c",
    "usermem_r.c",
    "userprintf_r.c",
    "userprintf_rbox_r.c",
    "rboxlib_r.c",
];

fn main() {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"),
    );

    let qhull_dir = manifest_dir.join("vendor/qhull/src/libqhull_r");

    if !qhull_dir.join("libqhull_r.c").exists() {
        panic!(
            "qhull source not found under crates/labdessem-fpha/vendor/qhull/src/libqhull_r/. \
             Run: git submodule update --init --recursive"
        );
    }

    println!("cargo:rerun-if-changed=vendor/qhull/src/libqhull_r/");

    let mut qhull_build = cc::Build::new();
    qhull_build.include(&qhull_dir);
    qhull_build.warnings(false);
    qhull_build.opt_level(2);

    for source in QHULL_SOURCES {
        let path = qhull_dir.join(source);
        if !path.exists() {
            panic!(
                "qhull source {source} not found. \
                 Run: git submodule update --init --recursive"
            );
        }
        qhull_build.file(path);
    }

    qhull_build.compile("qhull_r");

    let shim_dir = manifest_dir.join("csrc");
    let shim_source = shim_dir.join("qhull_wrapper.c");
    if !shim_source.exists() {
        panic!("C shim not found at crates/labdessem-fpha/csrc/qhull_wrapper.c");
    }

    println!("cargo:rerun-if-changed=csrc/qhull_wrapper.c");
    println!("cargo:rerun-if-changed=csrc/qhull_wrapper.h");

    let mut shim_build = cc::Build::new();
    shim_build.include(&qhull_dir);
    shim_build.include(&shim_dir);
    shim_build.warnings(true);
    shim_build.opt_level(2);
    shim_build.file(&shim_source);
    shim_build.compile("cobre_qhull_shim");
}
