//! Network output writers.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::IoError;
use crate::output::{AddedFlowCutLineDto, CmbRecordDto, LineFlowResultDto};

/// Write network flow results to CSV.
///
/// Produces `resultado_rede.csv` with columns:
/// `Linha;BarraDe;BarraPara;Periodo;FluxoMW;LimiteMW;Violacao`
pub fn write_network_csv(
    line_flows: &[LineFlowResultDto],
    output_dir: impl AsRef<Path>,
) -> Result<(), IoError> {
    let path = output_dir.as_ref().join("resultado_rede.csv");
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "Linha;BarraDe;BarraPara;Periodo;FluxoMW;LimiteMW;Violacao"
    )?;

    for flow in line_flows {
        writeln!(
            writer,
            "{};{};{};{};{:.4};{:.4};{}",
            flow.branch_name,
            flow.from_bus_name,
            flow.to_bus_name,
            flow.original_period,
            flow.flow_mw,
            flow.limit_mw,
            i32::from(flow.violated)
        )?;
    }

    writer.flush()?;
    Ok(())
}

/// Write added flow cut lines to CSV.
///
/// Produces `resultado_linhas_adicionadas.csv` with columns:
/// `NomeLinha;CodigoLinha;BarraDe;BarraPara;FluxoMW;CapacidadeMW`
pub fn write_added_flow_cut_lines_csv(
    added_lines: &[AddedFlowCutLineDto],
    output_dir: impl AsRef<Path>,
) -> Result<(), IoError> {
    let path = output_dir.as_ref().join("resultado_linhas_adicionadas.csv");
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "NomeLinha;CodigoLinha;BarraDe;BarraPara;FluxoMW;CapacidadeMW"
    )?;

    for line in added_lines {
        writeln!(
            writer,
            "{};{};{};{};{:.4};{:.4}",
            line.branch_name,
            line.branch_id,
            line.from_bus_id,
            line.to_bus_id,
            line.flow_mw,
            line.limit_mw
        )?;
    }

    writer.flush()?;
    Ok(())
}

/// Write nodal marginal costs (CMB) to CSV.
///
/// Produces `resultado_cmobar.csv` with columns:
/// `Periodo;CodigoBarra;NomeBarra;Submercado;CMB`
pub fn write_cmb_csv(
    cmb_records: &[CmbRecordDto],
    output_dir: impl AsRef<Path>,
) -> Result<(), IoError> {
    let path = output_dir.as_ref().join("resultado_cmobar.csv");
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "Periodo;CodigoBarra;NomeBarra;Submercado;CMB")?;

    for record in cmb_records {
        writeln!(
            writer,
            "{};{};{};{};{:.4}",
            record.original_period,
            record.bus_id,
            record.bus_name,
            record.submarket_name,
            record.cmb
        )?;
    }

    writer.flush()?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn read_file_lines(path: &Path) -> Vec<String> {
        fs::read_to_string(path)
            .expect("read file")
            .lines()
            .map(std::string::ToString::to_string)
            .collect()
    }

    #[test]
    fn write_network_csv_produces_correct_format() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");

        let flows = vec![
            LineFlowResultDto {
                branch_name: "L1".to_string(),
                from_bus_name: "B1".to_string(),
                to_bus_name: "B2".to_string(),
                original_period: 1,
                flow_mw: 150.0,
                limit_mw: 200.0,
                violated: false,
            },
            LineFlowResultDto {
                branch_name: "L2".to_string(),
                from_bus_name: "B2".to_string(),
                to_bus_name: "B3".to_string(),
                original_period: 2,
                flow_mw: 250.0,
                limit_mw: 200.0,
                violated: true,
            },
        ];

        write_network_csv(&flows, temp_dir.path()).expect("write network csv");

        let lines = read_file_lines(&temp_dir.path().join("resultado_rede.csv"));
        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[0],
            "Linha;BarraDe;BarraPara;Periodo;FluxoMW;LimiteMW;Violacao"
        );
        assert_eq!(lines[1], "L1;B1;B2;1;150.0000;200.0000;0");
        assert_eq!(lines[2], "L2;B2;B3;2;250.0000;200.0000;1");
    }

    #[test]
    fn write_added_flow_cut_lines_csv_produces_correct_format() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");

        let added_lines = vec![
            AddedFlowCutLineDto {
                branch_name: "L1".to_string(),
                branch_id: 0,
                from_bus_id: 0,
                to_bus_id: 1,
                flow_mw: 150.0,
                limit_mw: 200.0,
            },
            AddedFlowCutLineDto {
                branch_name: "L2".to_string(),
                branch_id: 1,
                from_bus_id: 1,
                to_bus_id: 2,
                flow_mw: 250.0,
                limit_mw: 200.0,
            },
        ];

        write_added_flow_cut_lines_csv(&added_lines, temp_dir.path())
            .expect("write added flow cut lines csv");

        let lines = read_file_lines(&temp_dir.path().join("resultado_linhas_adicionadas.csv"));
        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[0],
            "NomeLinha;CodigoLinha;BarraDe;BarraPara;FluxoMW;CapacidadeMW"
        );
        assert_eq!(lines[1], "L1;0;0;1;150.0000;200.0000");
        assert_eq!(lines[2], "L2;1;1;2;250.0000;200.0000");
    }

    #[test]
    fn write_cmb_csv_produces_correct_format() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");

        let cmb_records = vec![
            CmbRecordDto {
                original_period: 1,
                bus_id: 0,
                bus_name: "BUS1".to_string(),
                submarket_name: "SE".to_string(),
                cmb: 123.45,
            },
            CmbRecordDto {
                original_period: 2,
                bus_id: 1,
                bus_name: "BUS2".to_string(),
                submarket_name: "S".to_string(),
                cmb: 234.56,
            },
        ];

        write_cmb_csv(&cmb_records, temp_dir.path()).expect("write cmb csv");

        let lines = read_file_lines(&temp_dir.path().join("resultado_cmobar.csv"));
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Periodo;CodigoBarra;NomeBarra;Submercado;CMB");
        assert_eq!(lines[1], "1;0;BUS1;SE;123.4500");
        assert_eq!(lines[2], "2;1;BUS2;S;234.5600");
    }

    #[test]
    fn write_network_csv_with_empty_data_produces_header_only() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let flows: Vec<LineFlowResultDto> = vec![];

        write_network_csv(&flows, temp_dir.path()).expect("write network csv");

        let lines = read_file_lines(&temp_dir.path().join("resultado_rede.csv"));
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0],
            "Linha;BarraDe;BarraPara;Periodo;FluxoMW;LimiteMW;Violacao"
        );
    }
}
