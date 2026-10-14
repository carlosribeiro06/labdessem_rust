use std::{fs, path::Path};

use labdessem_core::system::FutureCostCut;

use crate::error::IoError;

pub fn read_future_cost_cuts(path: impl AsRef<Path>) -> Result<Vec<FutureCostCut>, IoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        IoError::invalid_data(format!(
            "failed to read future cost binary {}: {error}",
            path.display()
        ))
    })?;
    if bytes.len() < 8 {
        return Err(IoError::invalid_data(format!(
            "future cost binary {} is too small to be a valid FlatBuffers buffer",
            path.display()
        )));
    }

    let root_pos = read_fb_u32(&bytes, 0)? as usize;
    let stage_table = fb_table(&bytes, root_pos, "StageCuts")?;
    let stage_id = fb_get_u32(&bytes, &stage_table, 0)?.unwrap_or(0) as usize;
    let cuts_vector_pos = fb_get_vector_pos(&bytes, &stage_table, 4)?.ok_or_else(|| {
        IoError::invalid_data(format!(
            "future cost binary {} does not contain a cuts vector",
            path.display()
        ))
    })?;
    let cut_count = fb_vector_len(&bytes, cuts_vector_pos)?;
    let mut cuts = Vec::with_capacity(cut_count);

    for cut_idx in 0..cut_count {
        let cut_table_pos = fb_get_table_from_vector(&bytes, cuts_vector_pos, cut_idx)?;
        let cut_table = fb_table(&bytes, cut_table_pos, "Cut")?;
        let coefficients = if let Some(coeff_vector_pos) = fb_get_vector_pos(&bytes, &cut_table, 6)?
        {
            let coefficient_count = fb_vector_len(&bytes, coeff_vector_pos)?;
            (0..coefficient_count)
                .map(|coefficient_idx| {
                    fb_get_f64_from_vector(&bytes, coeff_vector_pos, coefficient_idx)
                })
                .collect::<Result<Vec<_>, IoError>>()?
        } else {
            Vec::new()
        };

        cuts.push(FutureCostCut {
            stage_id,
            cut_id: fb_get_u64(&bytes, &cut_table, 0)?.unwrap_or(0),
            slot_index: fb_get_u32(&bytes, &cut_table, 1)?.unwrap_or(cut_idx as u32) as usize,
            iteration: fb_get_u32(&bytes, &cut_table, 2)?.unwrap_or(0) as usize,
            forward_pass_index: fb_get_u32(&bytes, &cut_table, 3)?.unwrap_or(0) as usize,
            intercept: fb_get_f64(&bytes, &cut_table, 5)?.unwrap_or(0.0),
            coefficients,
            is_active: fb_get_bool(&bytes, &cut_table, 8)?.unwrap_or(false),
        });
    }

    Ok(cuts)
}

#[derive(Debug, Clone, Copy)]
struct FbTable {
    table_pos: usize,
    vtable_pos: usize,
}

fn fb_table(bytes: &[u8], table_pos: usize, label: &str) -> Result<FbTable, IoError> {
    let vtable_distance = read_fb_i32(bytes, table_pos)?;
    let vtable_pos = if vtable_distance >= 0 {
        table_pos
            .checked_sub(vtable_distance as usize)
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "{label} table at position {table_pos} points to an invalid vtable"
                ))
            })?
    } else {
        table_pos
            .checked_add(vtable_distance.unsigned_abs() as usize)
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "{label} table at position {table_pos} points to an invalid vtable"
                ))
            })?
    };
    let _ = read_fb_u16(bytes, vtable_pos)?;
    let _ = read_fb_u16(bytes, vtable_pos + 2)?;

    Ok(FbTable {
        table_pos,
        vtable_pos,
    })
}

fn fb_field_offset(
    bytes: &[u8],
    table: &FbTable,
    field_id: usize,
) -> Result<Option<usize>, IoError> {
    let field_pos = table.vtable_pos + 4 + field_id * 2;
    if field_pos + 2 > bytes.len() {
        return Ok(None);
    }
    let offset = read_fb_u16(bytes, field_pos)? as usize;
    if offset == 0 {
        Ok(None)
    } else {
        Ok(Some(table.table_pos + offset))
    }
}

fn fb_get_u32(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<u32>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_u32(bytes, pos))
        .transpose()
}

fn fb_get_u64(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<u64>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_u64(bytes, pos))
        .transpose()
}

fn fb_get_f64(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<f64>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_f64(bytes, pos))
        .transpose()
}

fn fb_get_bool(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<bool>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_bool(bytes, pos))
        .transpose()
}

fn fb_get_vector_pos(
    bytes: &[u8],
    table: &FbTable,
    field_id: usize,
) -> Result<Option<usize>, IoError> {
    let Some(field_pos) = fb_field_offset(bytes, table, field_id)? else {
        return Ok(None);
    };
    let vector_offset = read_fb_u32(bytes, field_pos)? as usize;
    Ok(Some(field_pos + vector_offset))
}

fn fb_vector_len(bytes: &[u8], vector_pos: usize) -> Result<usize, IoError> {
    Ok(read_fb_u32(bytes, vector_pos)? as usize)
}

fn fb_get_table_from_vector(
    bytes: &[u8],
    vector_pos: usize,
    index: usize,
) -> Result<usize, IoError> {
    let element_pos = vector_pos + 4 + index * 4;
    let relative = read_fb_u32(bytes, element_pos)? as usize;
    Ok(element_pos + relative)
}

fn fb_get_f64_from_vector(bytes: &[u8], vector_pos: usize, index: usize) -> Result<f64, IoError> {
    let element_pos = vector_pos + 4 + index * 8;
    read_fb_f64(bytes, element_pos)
}

fn read_fb_u16(bytes: &[u8], pos: usize) -> Result<u16, IoError> {
    let slice = bytes.get(pos..pos + 2).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 2
        ))
    })?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_fb_u32(bytes: &[u8], pos: usize) -> Result<u32, IoError> {
    let slice = bytes.get(pos..pos + 4).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 4
        ))
    })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_fb_i32(bytes: &[u8], pos: usize) -> Result<i32, IoError> {
    let slice = bytes.get(pos..pos + 4).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 4
        ))
    })?;
    Ok(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_fb_u64(bytes: &[u8], pos: usize) -> Result<u64, IoError> {
    let slice = bytes.get(pos..pos + 8).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 8
        ))
    })?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_fb_f64(bytes: &[u8], pos: usize) -> Result<f64, IoError> {
    let slice = bytes.get(pos..pos + 8).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 8
        ))
    })?;
    Ok(f64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_fb_bool(bytes: &[u8], pos: usize) -> Result<bool, IoError> {
    let value = *bytes.get(pos).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte index {pos}"
        ))
    })?;
    Ok(value != 0)
}
