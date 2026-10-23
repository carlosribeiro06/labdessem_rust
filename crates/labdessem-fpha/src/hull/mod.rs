//! Safe Rust wrapper over the qhull 3-D convex-hull C shim.

mod ffi;

use std::os::raw::{c_double, c_int};
use thiserror::Error;

/// A facet hyperplane of a 3-D convex hull.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hyperplane3d {
    /// Unit-length outward normal (nx, ny, nz).
    pub normal: [f64; 3],
    /// Plane offset d in nx·x + ny·y + nz·z + d = 0.
    pub offset: f64,
}

/// Failure modes of convex_hull_3d.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HullError {
    #[error("convex hull input contains a non-finite coordinate")]
    NonFiniteInput,

    #[error("convex hull input is degenerate or has fewer than 4 affinely-independent points")]
    Degenerate,

    #[error("qhull convex-hull computation failed")]
    Compute,

    #[error("convex hull allocation failed")]
    Alloc,

    #[error("qhull shim returned unknown status code {0}")]
    UnknownStatus(c_int),
}

/// RAII owner of the shim-malloc'd plane buffer.
struct PlaneBuffer {
    ptr: *mut c_double,
}

impl Drop for PlaneBuffer {
    fn drop(&mut self) {
        // SAFETY: ptr is either NULL or from the shim; freed exactly once.
        unsafe { ffi::labdessem_qhull_free(self.ptr) };
    }
}

/// Compute the 3-D convex hull of points, returning facet hyperplanes in canonical order.
///
/// Determinism: input sorted before FFI, output sorted after. Two clouds that
/// are permutations of each other produce bit-identical Vec<Hyperplane3d>.
pub fn convex_hull_3d(points: &[[f64; 3]]) -> Result<Vec<Hyperplane3d>, HullError> {
    if points.iter().any(|p| p.iter().any(|c| !c.is_finite())) {
        return Err(HullError::NonFiniteInput);
    }

    let mut sorted: Vec<[f64; 3]> = points.to_vec();
    sorted.sort_by(|a, b| {
        a[0].total_cmp(&b[0])
            .then_with(|| a[1].total_cmp(&b[1]))
            .then_with(|| a[2].total_cmp(&b[2]))
    });

    let n_points = sorted.len();
    let mut flat: Vec<f64> = Vec::with_capacity(n_points * 3);
    for p in &sorted {
        flat.extend_from_slice(p);
    }

    let Ok(n_points_c) = c_int::try_from(n_points) else {
        return Err(HullError::Compute);
    };

    let mut out_planes: *mut c_double = std::ptr::null_mut();
    let mut out_n_facets: c_int = 0;

    // SAFETY: flat is valid for 3*n_points reads, out_planes/out_n_facets are live locals.
    let status = unsafe {
        ffi::labdessem_qhull_convex_hull_3d(
            flat.as_ptr(),
            n_points_c,
            std::ptr::addr_of_mut!(out_planes),
            std::ptr::addr_of_mut!(out_n_facets),
        )
    };

    if status != ffi::LABDESSEM_QHULL_OK {
        return Err(match status {
            ffi::LABDESSEM_QHULL_ERR_DEGENERATE => HullError::Degenerate,
            ffi::LABDESSEM_QHULL_ERR_COMPUTE => HullError::Compute,
            ffi::LABDESSEM_QHULL_ERR_ALLOC => HullError::Alloc,
            other => HullError::UnknownStatus(other),
        });
    }

    let buffer = PlaneBuffer { ptr: out_planes };

    let Ok(n_facets) = usize::try_from(out_n_facets) else {
        return Err(HullError::Compute);
    };

    let mut facets: Vec<Hyperplane3d> = Vec::with_capacity(n_facets);
    if n_facets > 0 {
        // SAFETY: buffer.ptr is non-NULL on OK status, layout is 4*n_facets f64s.
        let planes: &[c_double] = unsafe { std::slice::from_raw_parts(buffer.ptr, n_facets * 4) };
        for q in planes.chunks_exact(4) {
            facets.push(Hyperplane3d {
                normal: [q[0], q[1], q[2]],
                offset: q[3],
            });
        }
    }

    facets.sort_by(|a, b| {
        a.normal[0]
            .total_cmp(&b.normal[0])
            .then_with(|| a.normal[1].total_cmp(&b.normal[1]))
            .then_with(|| a.normal[2].total_cmp(&b.normal[2]))
            .then_with(|| a.offset.total_cmp(&b.offset))
    });

    Ok(facets)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::{convex_hull_3d, HullError, Hyperplane3d};

    const TETRA: [[f64; 3]; 4] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];

    fn matches_plane(plane: &Hyperplane3d, nx: f64, ny: f64, nz: f64, d: f64) -> bool {
        let want = [nx, ny, nz];
        let want_norm = (nx * nx + ny * ny + nz * nz).sqrt();
        let got = plane.normal;
        let got_norm = (got[0] * got[0] + got[1] * got[1] + got[2] * got[2]).sqrt();
        if got_norm == 0.0 || want_norm == 0.0 {
            return false;
        }
        for sign in [1.0_f64, -1.0_f64] {
            let normal_matches =
                (0..3).all(|i| (got[i] / got_norm - sign * want[i] / want_norm).abs() < 1e-9);
            let offset_matches = (plane.offset / got_norm - sign * d / want_norm).abs() < 1e-9;
            if normal_matches && offset_matches {
                return true;
            }
        }
        false
    }

    #[test]
    fn tetrahedron_returns_four_faces() {
        let hull = convex_hull_3d(&TETRA).expect("tetrahedron is valid");
        assert_eq!(hull.len(), 4);

        let expected = [
            (1.0, 0.0, 0.0, 0.0),
            (0.0, 1.0, 0.0, 0.0),
            (0.0, 0.0, 1.0, 0.0),
            (1.0, 1.0, 1.0, -1.0),
        ];

        for (nx, ny, nz, d) in expected {
            assert!(
                hull.iter().any(|p| matches_plane(p, nx, ny, nz, d)),
                "missing plane ({nx},{ny},{nz},{d})"
            );
        }
    }

    #[test]
    fn shuffled_input_yields_bit_identical_facets() {
        let baseline = convex_hull_3d(&TETRA).expect("baseline");
        let shuffled = [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let permuted = convex_hull_3d(&shuffled).expect("shuffled");

        assert_eq!(baseline.len(), permuted.len());
        for (a, b) in baseline.iter().zip(&permuted) {
            assert_eq!(a.normal[0].to_bits(), b.normal[0].to_bits());
            assert_eq!(a.normal[1].to_bits(), b.normal[1].to_bits());
            assert_eq!(a.normal[2].to_bits(), b.normal[2].to_bits());
            assert_eq!(a.offset.to_bits(), b.offset.to_bits());
        }
    }

    #[test]
    fn coplanar_points_return_degenerate_error() {
        let coplanar = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(convex_hull_3d(&coplanar), Err(HullError::Degenerate));
    }

    #[test]
    fn nan_input_returns_non_finite_error() {
        let with_nan = [
            [0.0, f64::NAN, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        assert_eq!(convex_hull_3d(&with_nan), Err(HullError::NonFiniteInput));
    }
}
