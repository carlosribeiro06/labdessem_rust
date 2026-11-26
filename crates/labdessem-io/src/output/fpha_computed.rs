//! Writer for computed FPHA results.

use std::path::Path;

use labdessem_core::system::System;
use labdessem_model::production::FphaPlantResult;

use crate::error::IoError;
use crate::output::{csv_with_header, write_csv_file};

/// Write computed FPHA planes to `RESULTADOS/resultado_fpha_computed.csv`: one row per plane, in
/// `fits` order then plane order, named after the plant of `system` that has the fit's id.
///
/// Creates the `RESULTADOS` directory if it does not exist.
/// Overwrites the file if it already exists.
/// A fit whose plant is not in `system` is an error, and the file is not written.
pub fn write_fpha_computed(
    case_path: &Path,
    system: &System,
    fits: &[FphaPlantResult],
) -> Result<(), IoError> {
    let results_dir = case_path.join("RESULTADOS");
    std::fs::create_dir_all(&results_dir).map_err(|e| {
        IoError::invalid_data(format!(
            "failed to create RESULTADOS directory at {}: {e}",
            results_dir.display()
        ))
    })?;

    let mut csv = csv_with_header(
        "codigo_usina;nome_usina;plane_index;gamma_0;gamma_v;gamma_q;gamma_s;alpha;max_deviation_mw;mean_deviation_mw;rms_deviation_mw",
        &[
            (
                "codigo_usina",
                "codigo identificador da usina hidroeletrica",
            ),
            ("nome_usina", "nome da usina hidroeletrica"),
            ("plane_index", "indice do plano FPHA (0-indexed)"),
            (
                "gamma_0",
                "coeficiente independente do hiperplano (RHS em MW), sem o fator alpha",
            ),
            (
                "gamma_v",
                "coeficiente do volume armazenado (MW/hm3), sem o fator alpha",
            ),
            (
                "gamma_q",
                "coeficiente da vazao turbinada (MW/m3/s), sem o fator alpha",
            ),
            (
                "gamma_s",
                "coeficiente do vertimento (MW/m3/s), sem o fator alpha",
            ),
            (
                "alpha",
                "fator de correcao que o modelo aplica uma vez aos quatro coeficientes (Fcorrec do OPER_FPHA)",
            ),
            (
                "max_deviation_mw",
                "desvio maximo absoluto da aproximacao (MW) de alpha x plano em relacao a FPH",
            ),
            (
                "mean_deviation_mw",
                "desvio medio absoluto da aproximacao (MW) de alpha x plano em relacao a FPH",
            ),
            (
                "rms_deviation_mw",
                "raiz do erro quadratico medio da aproximacao (MW) de alpha x plano em relacao a FPH",
            ),
        ],
    );

    for fit in fits {
        let plant = system
            .hydro_plants
            .iter()
            .find(|plant| plant.id == fit.id)
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "computed FPHA for hydro plant {} that is not in the system",
                    fit.id.0
                ))
            })?;
        let deviation = &fit.deviation;
        for (plane_index, plane) in fit.planes.iter().enumerate() {
            csv.push_str(&format!(
                "{};{};{};{:.6};{:.9};{:.6};{:.9};{:.6};{:.3};{:.3};{:.3}\n",
                fit.id.0,
                plant.name,
                plane_index,
                plane.gamma_0,
                plane.gamma_v,
                plane.gamma_q,
                plane.gamma_s,
                fit.alpha,
                deviation.max_abs_mw,
                deviation.mean_abs_mw,
                deviation.rms_mw,
            ));
        }
    }

    let file_path = results_dir.join("resultado_fpha_computed.csv");
    write_csv_file(&file_path, csv).map_err(|e| {
        IoError::invalid_data(format!("failed to write {}: {e}", file_path.display()))
    })?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::fs;

    use labdessem_core::ids::HydroPlantId;
    use labdessem_core::test_support::{minimal_hydro, minimal_system};
    use labdessem_model::production::{FphaFitDeviation, RawPlane};
    use tempfile::TempDir;

    use super::{FphaPlantResult, System, write_fpha_computed};

    const HEADER: &str = "\
ONS - Operador Nacional do Sistema Eletrico
LabDessem

Descricao das colunas:
codigo_usina: codigo identificador da usina hidroeletrica
nome_usina: nome da usina hidroeletrica
plane_index: indice do plano FPHA (0-indexed)
gamma_0: coeficiente independente do hiperplano (RHS em MW), sem o fator alpha
gamma_v: coeficiente do volume armazenado (MW/hm3), sem o fator alpha
gamma_q: coeficiente da vazao turbinada (MW/m3/s), sem o fator alpha
gamma_s: coeficiente do vertimento (MW/m3/s), sem o fator alpha
alpha: fator de correcao que o modelo aplica uma vez aos quatro coeficientes (Fcorrec do OPER_FPHA)
max_deviation_mw: desvio maximo absoluto da aproximacao (MW) de alpha x plano em relacao a FPH
mean_deviation_mw: desvio medio absoluto da aproximacao (MW) de alpha x plano em relacao a FPH
rms_deviation_mw: raiz do erro quadratico medio da aproximacao (MW) de alpha x plano em relacao a FPH

codigo_usina;nome_usina;plane_index;gamma_0;gamma_v;gamma_q;gamma_s;alpha;max_deviation_mw;mean_deviation_mw;rms_deviation_mw
";

    fn system_with(plants: &[(usize, &str)]) -> System {
        let mut system = minimal_system();
        system.hydro_plants = plants
            .iter()
            .map(|&(id, name)| {
                let mut plant = minimal_hydro(id, 1, 1, 1.0, 0.1, 10.0, 1);
                plant.name = name.to_string();
                plant
            })
            .collect();
        system
    }

    fn fit(id: usize, planes: &[[f64; 4]], alpha: f64, deviation_mw: [f64; 3]) -> FphaPlantResult {
        let [max_abs_mw, mean_abs_mw, rms_mw] = deviation_mw;
        FphaPlantResult {
            id: HydroPlantId(id),
            name: format!("FIT-{id}"),
            planes: planes
                .iter()
                .map(|&[gamma_0, gamma_v, gamma_q, gamma_s]| RawPlane {
                    gamma_0,
                    gamma_v,
                    gamma_q,
                    gamma_s,
                })
                .collect(),
            alpha,
            deviation: FphaFitDeviation {
                mean_abs_mw,
                max_abs_mw,
                rms_mw,
                mean_signed_mw: 0.0,
                relative: 0.0,
            },
        }
    }

    fn written(dir: &TempDir) -> String {
        fs::read_to_string(dir.path().join("RESULTADOS/resultado_fpha_computed.csv")).unwrap()
    }

    #[test]
    fn write_fpha_computed_creates_file() {
        let dir = TempDir::new().unwrap();
        let system = system_with(&[(1, "FURNAS")]);
        let fits = [fit(
            1,
            &[[1234.5, -0.0023, 0.045, 0.001]],
            0.95,
            [1.5, 0.8, 0.9],
        )];

        write_fpha_computed(dir.path(), &system, &fits).unwrap();

        assert_eq!(
            written(&dir),
            format!(
                "{HEADER}1;FURNAS;0;1234.500000;-0.002300000;0.045000;0.001000000;0.950000;1.500;0.800;0.900\n"
            )
        );
    }

    #[test]
    fn write_fpha_computed_creates_directory() {
        let dir = TempDir::new().unwrap();
        assert!(!dir.path().join("RESULTADOS").exists());

        write_fpha_computed(dir.path(), &system_with(&[]), &[]).unwrap();

        assert!(dir.path().join("RESULTADOS").exists());
    }

    #[test]
    fn write_fpha_computed_multiple_plants() {
        let dir = TempDir::new().unwrap();
        let system = system_with(&[(1, "PLANT_A"), (2, "PLANT_B")]);
        let fits = [
            fit(
                1,
                &[[100.0, 0.1, 0.2, 0.3], [90.0, 0.08, 0.18, 0.25]],
                0.9,
                [1.0, 0.5, 0.6],
            ),
            fit(2, &[[200.0, 0.4, 0.5, 0.6]], 0.8, [2.0, 1.2, 1.4]),
        ];

        write_fpha_computed(dir.path(), &system, &fits).unwrap();

        assert_eq!(
            written(&dir),
            format!(
                "{HEADER}\
                 1;PLANT_A;0;100.000000;0.100000000;0.200000;0.300000000;0.900000;1.000;0.500;0.600\n\
                 1;PLANT_A;1;90.000000;0.080000000;0.180000;0.250000000;0.900000;1.000;0.500;0.600\n\
                 2;PLANT_B;0;200.000000;0.400000000;0.500000;0.600000000;0.800000;2.000;1.200;1.400\n"
            )
        );
    }

    #[test]
    fn write_fpha_computed_overwrites() {
        let dir = TempDir::new().unwrap();
        let system = system_with(&[(1, "FIRST"), (2, "SECOND")]);

        write_fpha_computed(
            dir.path(),
            &system,
            &[fit(1, &[[1.0, 0.0, 0.0, 0.0]], 1.0, [0.0; 3])],
        )
        .unwrap();
        write_fpha_computed(
            dir.path(),
            &system,
            &[fit(2, &[[2.0, 0.0, 0.0, 0.0]], 1.0, [0.0; 3])],
        )
        .unwrap();

        assert_eq!(
            written(&dir),
            format!(
                "{HEADER}2;SECOND;0;2.000000;0.000000000;0.000000;0.000000000;1.000000;0.000;0.000;0.000\n"
            )
        );
    }

    #[test]
    fn write_fpha_computed_empty_records() {
        let dir = TempDir::new().unwrap();

        write_fpha_computed(dir.path(), &system_with(&[]), &[]).unwrap();

        assert_eq!(written(&dir), HEADER);
    }

    #[test]
    fn write_fpha_computed_with_populated_deviations() {
        let dir = TempDir::new().unwrap();
        let system = system_with(&[(1, "TEST")]);
        let fits = [fit(
            1,
            &[[100.0, 0.1, 0.2, 0.3]],
            0.9,
            [1.234, 0.567, 0.789],
        )];

        write_fpha_computed(dir.path(), &system, &fits).unwrap();

        assert_eq!(
            written(&dir),
            format!(
                "{HEADER}1;TEST;0;100.000000;0.100000000;0.200000;0.300000000;0.900000;1.234;0.567;0.789\n"
            )
        );
    }

    #[test]
    fn write_fpha_computed_header_matches_expected_columns() {
        let dir = TempDir::new().unwrap();

        write_fpha_computed(dir.path(), &system_with(&[]), &[]).unwrap();

        let content = written(&dir);
        // Find the column header line (contains codigo_usina;nome_usina)
        let header = content
            .lines()
            .find(|l| l.starts_with("codigo_usina;"))
            .expect("Column header line not found");

        let expected_columns = [
            "codigo_usina",
            "nome_usina",
            "plane_index",
            "gamma_0",
            "gamma_v",
            "gamma_q",
            "gamma_s",
            "alpha",
            "max_deviation_mw",
            "mean_deviation_mw",
            "rms_deviation_mw",
        ];
        let actual_columns: Vec<&str> = header.split(';').collect();
        assert_eq!(actual_columns, expected_columns);
    }

    #[test]
    fn write_fpha_computed_rejects_a_fit_for_an_unknown_plant() {
        let dir = TempDir::new().unwrap();
        let system = system_with(&[(1, "FURNAS")]);
        let fits = [fit(2, &[[100.0, 0.1, 0.2, 0.3]], 0.9, [0.0; 3])];

        let error = write_fpha_computed(dir.path(), &system, &fits)
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("computed FPHA for hydro plant 2 that is not in the system"),
            "{error}"
        );
        assert!(
            !dir.path()
                .join("RESULTADOS/resultado_fpha_computed.csv")
                .exists()
        );
    }
}
