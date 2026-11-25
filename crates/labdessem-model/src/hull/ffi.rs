//! Raw FFI binding to the qhull C shim.

use std::os::raw::{c_double, c_int};

pub const LABDESSEM_QHULL_OK: c_int = 0;
pub const LABDESSEM_QHULL_ERR_COMPUTE: c_int = 1;
pub const LABDESSEM_QHULL_ERR_DEGENERATE: c_int = 2;
pub const LABDESSEM_QHULL_ERR_ALLOC: c_int = 3;

unsafe extern "C" {
    pub fn labdessem_qhull_convex_hull_3d(
        points: *const c_double,
        n_points: c_int,
        out_planes: *mut *mut c_double,
        out_n_facets: *mut c_int,
    ) -> c_int;

    pub fn labdessem_qhull_free(planes: *mut c_double);
}
