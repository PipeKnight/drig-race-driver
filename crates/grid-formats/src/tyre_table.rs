//! Ordered tyre-grip CSV columns and fixed original row-code slots.
use crate::{Error, Result, fail};

pub const ROW_CODES: [[u8; 4]; 13] = [
    *b"LIGR", *b"MDGR", *b"HEGR", *b"DUGR", *b"LPGR", *b"PEGR", *b"WTGR", *b"INGR", *b"MUDD",
    *b"SMTM", *b"ROTM", *b"WTTM", *b"INTM",
];

#[derive(Clone, Debug)]
pub struct Table {
    pub columns: Vec<[u8; 4]>,
    /// First matching source row occupies its fixed slot; absence stays explicit.
    pub rows: [Option<Vec<f32>>; 13],
}

impl Table {
    pub fn active(&self) -> bool {
        !self.columns.is_empty()
    }
}

pub fn read(bytes: &[u8]) -> Result<Table> {
    if bytes.len() > 2 * 1024 * 1024 || bytes.is_empty() {
        return fail(0, "invalid tyre CSV size");
    }
    if !bytes.is_ascii() || bytes.iter().any(|b| *b == 0 || *b == b'"') {
        return fail(0, "unsupported tyre CSV text");
    }
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' && (i == 0 || bytes[i - 1] != b'\r') {
            return fail(i, "tyre CSV requires CR or CRLF row delimiters");
        }
    }
    let text = std::str::from_utf8(bytes).unwrap();
    let records: Vec<Vec<&str>> = text
        .split('\r')
        .map(|line| line.strip_prefix('\n').unwrap_or(line).split(',').collect())
        .collect();
    if records.len() < 3
        || records.len() > 256
        || records
            .iter()
            .any(|r| r.len() > 360 || r.iter().any(|v| v.len() > 63))
    {
        return fail(0, "unsupported tyre CSV row or cell count");
    }
    let columns: Vec<_> = records[0]
        .iter()
        .skip(2)
        .take_while(|v| !v.is_empty())
        .map(|v| {
            if v.len() < 4 {
                [0xff; 4]
            } else {
                v.as_bytes()[..4].try_into().unwrap()
            }
        })
        .collect();
    let mut rows: [Option<Vec<f32>>; 13] = std::array::from_fn(|_| None);
    if !columns.is_empty() {
        for record in records.iter().skip(2).take_while(|r| !r[0].is_empty()) {
            if record[0].len() < 4 {
                continue;
            }
            let code: [u8; 4] = record[0].as_bytes()[..4].try_into().unwrap();
            let Some(slot) = ROW_CODES.iter().position(|k| *k == code) else {
                continue;
            };
            if rows[slot].is_some() {
                continue;
            }
            let values = (0..columns.len())
                .map(|i| -> Result<f32> {
                    let text = record.get(i + 2).copied().unwrap_or("");
                    let number: f64 = if text.is_empty() {
                        0.
                    } else {
                        text.parse().map_err(|_| Error {
                            offset: i + 2,
                            message: "invalid tyre table number",
                        })?
                    };
                    let value = (number * f32::from_bits(0x3c23_d70a) as f64) as f32;
                    if !number.is_finite() || !value.is_finite() {
                        return fail(i + 2, "nonfinite tyre table value");
                    }
                    Ok(value)
                })
                .collect::<Result<_>>()?;
            rows[slot] = Some(values);
        }
    }
    Ok(Table { columns, rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_slots_first_rows_padding_and_blank_row_termination() {
        let table = read(b",,TEST_more,XX,MUD ,\r\nlabels\r\nWTGR,,150,,50\r\nLIGR,,100,25\r\nWTGR,,900,900,900\r\n\r\nHEGR,,800,800,800\r\n").unwrap();
        assert_eq!(table.columns, [*b"TEST", [0xff; 4], *b"MUD "]);
        assert!(table.rows[2].is_none());
        assert_eq!(table.rows[6].as_ref().unwrap(), &[1.5, 0., 0.5]);
        assert_eq!(table.rows[0].as_ref().unwrap(), &[1., 0.25, 0.]);
        assert!(read(b",,TEST\nlabels\nLIGR,,100\n").is_err());
        assert!(read(b",,TEST\r\nlabels\r\nLIGR,,NaN\r\n").is_err());
    }
}
