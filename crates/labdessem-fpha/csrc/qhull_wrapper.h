#ifndef LABDESSEM_QHULL_WRAPPER_H
#define LABDESSEM_QHULL_WRAPPER_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LABDESSEM_QHULL_OK 0
#define LABDESSEM_QHULL_ERR_COMPUTE 1
#define LABDESSEM_QHULL_ERR_DEGENERATE 2
#define LABDESSEM_QHULL_ERR_ALLOC 3

int labdessem_qhull_convex_hull_3d(
    const double* points,
    int n_points,
    double** out_planes,
    int* out_n_facets
);

void labdessem_qhull_free(double* planes);

#ifdef __cplusplus
}
#endif

#endif
