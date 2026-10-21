//! Process iteration reporting output.

use std::path::Path;

use crate::error::IoError;

use super::{IterationReportDto, csv_with_header, write_csv_file};

/// Write process iteration report to CSV.
///
/// Output file: `resultado_processo_iter.csv`
///
/// Columns:
/// - `Etapa`: iteration stage label
/// - `Iteracao`: iteration number within stage
/// - `NumeroRestricoes`: constraint count
/// - `NumeroVariaveis`: variable count
/// - `ValorFuncaoObjetivo`: objective value in R$
/// - `StatusSolucao`: solution status
/// - `TempoResolucaoSeg`: solve time in seconds
pub fn write_process_iterations_csv(
    steps: &[IterationReportDto],
    output_dir: impl AsRef<Path>,
) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Etapa;Iteracao;NumeroRestricoes;NumeroVariaveis;ValorFuncaoObjetivo;StatusSolucao;TempoResolucaoSeg",
        &[
            ("Etapa", "etapa do processo iterativo"),
            ("Iteracao", "numero da iteracao dentro da etapa"),
            (
                "NumeroRestricoes",
                "quantidade de restricoes do problema resolvido",
            ),
            (
                "NumeroVariaveis",
                "quantidade de variaveis do problema resolvido",
            ),
            ("ValorFuncaoObjetivo", "valor da funcao objetivo em R$"),
            ("StatusSolucao", "status retornado para a solucao"),
            ("TempoResolucaoSeg", "tempo de resolucao em segundos"),
        ],
    );

    for step in steps {
        csv.push_str(&format!(
            "{};{};{};{};{:.6};{};{:.6}\n",
            step.stage.label(),
            step.iteration,
            step.constraint_count,
            step.variable_count,
            step.objective_value,
            step.solution_status,
            step.solve_time_seconds
        ));
    }

    write_csv_file(output_dir.as_ref().join("resultado_processo_iter.csv"), csv)?;
    Ok(())
}
