#include "qhull_wrapper.h"

#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>

#include "qhull_ra.h"

#define DIM 3
#define PLANE_STRIDE 4

static const char QHULL_FLAGS[] = "qhull Qt Pp";

static int map_error(int exitcode) {
    switch (exitcode) {
        case qh_ERRmem:
            return LABDESSEM_QHULL_ERR_ALLOC;
        case qh_ERRsingular:
        case qh_ERRprec:
        case qh_ERRtopology:
        case qh_ERRwide:
            return LABDESSEM_QHULL_ERR_DEGENERATE;
        default:
            return LABDESSEM_QHULL_ERR_COMPUTE;
    }
}

int labdessem_qhull_convex_hull_3d(
    const double* points,
    int n_points,
    double** out_planes,
    int* out_n_facets
) {
    if (out_planes != NULL) *out_planes = NULL;
    if (out_n_facets != NULL) *out_n_facets = 0;

    if (points == NULL || out_planes == NULL || out_n_facets == NULL) {
        return LABDESSEM_QHULL_ERR_COMPUTE;
    }

    if (n_points < DIM + 1) {
        return LABDESSEM_QHULL_ERR_DEGENERATE;
    }

    QHULL_LIB_CHECK

    char* diag_buf = NULL;
    size_t diag_size = 0;
    FILE* diag_stream = NULL;
#if defined(_WIN32)
    diag_stream = tmpfile();
#else
    diag_stream = open_memstream(&diag_buf, &diag_size);
    if (diag_stream == NULL) diag_stream = tmpfile();
#endif
    FILE* err_stream = (diag_stream != NULL) ? diag_stream : stderr;

    qhT qh_qh;
    qhT* qh = &qh_qh;
    qh_zero(qh, err_stream);

    double* planes = NULL;
    int status = LABDESSEM_QHULL_OK;

    int exitcode = setjmp(qh->errexit);
    if (!exitcode) {
        qh->NOerrexit = False;

        int qhull_exit = qh_new_qhull(
            qh,
            DIM,
            n_points,
            (coordT*)points,
            False,
            (char*)QHULL_FLAGS,
            NULL,
            err_stream
        );

        facetT* facet = NULL;
        int n_facets = 0;
        FORALLfacets {
            if (facet->upperdelaunay || facet->normal == NULL) continue;
            n_facets++;
        }

        if (n_facets == 0) {
            status = (qhull_exit != 0) ? map_error(qhull_exit) : LABDESSEM_QHULL_ERR_DEGENERATE;
        } else {
            size_t count = (size_t)n_facets * (size_t)PLANE_STRIDE;
            planes = (double*)malloc(count * sizeof(double));
            if (planes == NULL) {
                status = LABDESSEM_QHULL_ERR_ALLOC;
            } else {
                int idx = 0;
                FORALLfacets {
                    if (facet->upperdelaunay || facet->normal == NULL) continue;
                    double* dst = planes + (size_t)idx * PLANE_STRIDE;
                    dst[0] = (double)facet->normal[0];
                    dst[1] = (double)facet->normal[1];
                    dst[2] = (double)facet->normal[2];
                    dst[3] = (double)facet->offset;
                    idx++;
                }
                *out_planes = planes;
                *out_n_facets = n_facets;
            }
        }
    } else {
        status = map_error(exitcode);
    }

    qh->NOerrexit = True;
    qh_freeqhull(qh, !qh_ALL);
    int curlong = 0, totlong = 0;
    qh_memfreeshort(qh, &curlong, &totlong);

    if (status != LABDESSEM_QHULL_OK && planes != NULL) {
        free(planes);
        *out_planes = NULL;
        *out_n_facets = 0;
    }

    if (diag_stream != NULL) fclose(diag_stream);
    free(diag_buf);

    return status;
}

void labdessem_qhull_free(double* planes) {
    free(planes);
}
