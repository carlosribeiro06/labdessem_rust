//! Infeasibility result writers.

use std::path::Path;

use crate::error::IoError;
use crate::output::{
    NetworkInfeasibilityDto, OperationalLimitInfeasibilityDto, csv_with_header,
    operational_limit_variable_label, write_csv_file,
};

/// Write network infeasibilities to CSV.
pub fn write_network_infeasibility_csv(
    infeasibilities: &[NetworkInfeasibilityDto],
    output_dir: &Path,
) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Estagio;Iteracao;Periodo;Ramo;Violacao_MW",
        &[
            ("Estagio", "estagio do processo iterativo"),
            ("Iteracao", "numero da iteracao"),
            ("Periodo", "periodo original"),
            ("Ramo", "identificador do ramo violado"),
            ("Violacao_MW", "magnitude da violacao em MW"),
        ],
    );

    for record in infeasibilities {
        csv.push_str(&format!(
            "{};{};{};{};{:.4}\n",
            record.stage.label(),
            record.iteration,
            record.period,
            record.branch_name,
            record.violation_mw
        ));
    }

    write_csv_file(output_dir.join("resultado_inviabilidade_rede.csv"), csv)?;
    Ok(())
}

/// Write operational limit infeasibilities to CSV.
pub fn write_operational_limit_infeasibility_csv(
    infeasibilities: &[OperationalLimitInfeasibilityDto],
    output_dir: &Path,
) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Estagio;Iteracao;Periodo;CodigoUsina;NomeUsina;Variavel;Unidade;ViolacaoInf;ViolacaoSup",
        &[
            ("Estagio", "estagio do processo iterativo"),
            ("Iteracao", "numero da iteracao"),
            ("Periodo", "periodo original"),
            ("CodigoUsina", "codigo da usina"),
            ("NomeUsina", "nome da usina"),
            ("Variavel", "variavel do limite operativo"),
            ("Unidade", "unidade de medida da variavel"),
            ("ViolacaoInf", "violacao do limite inferior"),
            ("ViolacaoSup", "violacao do limite superior"),
        ],
    );

    for record in infeasibilities {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{:.4};{:.4}\n",
            record.stage.label(),
            record.iteration,
            record.period,
            record.plant_code,
            record.plant_name,
            operational_limit_variable_label(record.variable),
            record.unit,
            record.lower_violation,
            record.upper_violation
        ));
    }

    write_csv_file(output_dir.join("resultado_inviabilidade_lim.csv"), csv)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{IterationStageLabel, OperationalLimitInfeasibilityDto};
    use labdessem_core::system::OperationalLimitVariable;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn write_network_infeasibility_csv_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let infeasibilities = vec![NetworkInfeasibilityDto {
            stage: IterationStageLabel::Milp,
            iteration: 1,
            period: 5,
            branch_name: "L01-02".to_string(),
            violation_mw: 12.5,
        }];

        let result = write_network_infeasibility_csv(&infeasibilities, temp_dir.path());
        assert!(result.is_ok());

        let file_path = temp_dir.path().join("resultado_inviabilidade_rede.csv");
        assert!(file_path.exists());

        let contents = fs::read_to_string(file_path).unwrap();
        assert!(contents.contains("MILP;1;5;L01-02;12.5000"));
    }

    #[test]
    fn write_operational_limit_infeasibility_csv_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let infeasibilities = vec![OperationalLimitInfeasibilityDto {
            stage: IterationStageLabel::LpFixed,
            iteration: 2,
            period: 10,
            plant_code: 123,
            plant_name: "PLANT_A".to_string(),
            variable: OperationalLimitVariable::Generation,
            unit: "MW",
            lower_violation: 5.0,
            upper_violation: 0.0,
        }];

        let result = write_operational_limit_infeasibility_csv(&infeasibilities, temp_dir.path());
        assert!(result.is_ok());

        let file_path = temp_dir.path().join("resultado_inviabilidade_lim.csv");
        assert!(file_path.exists());

        let contents = fs::read_to_string(file_path).unwrap();
        assert!(contents.contains("LP-FIXED;2;10;123;PLANT_A;GER;MW;5.0000;0.0000"));
    }
}
