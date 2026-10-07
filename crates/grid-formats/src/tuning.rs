//! GRID's unquoted comma-delimited tuning subset, retaining positional lexemes.
use crate::{Result, fail};
#[derive(Debug)]
pub struct Row<'a> {
    pub fields: Vec<&'a str>,
}
#[derive(Debug)]
pub struct Table<'a> {
    pub rows: Vec<Row<'a>>,
    pub blank_lines: usize,
}
impl Row<'_> {
    pub fn number(&self, index: usize) -> Result<Option<f64>> {
        let text = self.fields.get(index).ok_or(crate::Error {
            offset: index,
            message: "tuning column out of bounds",
        })?;
        if text.is_empty() {
            return Ok(None);
        }
        let value: f64 = text.parse().map_err(|_| crate::Error {
            offset: index,
            message: "tuning field is not numeric",
        })?;
        if !value.is_finite() {
            return fail(index, "nonfinite tuning field");
        }
        Ok(Some(value))
    }
}
pub fn read(bytes: &[u8]) -> Result<Table<'_>> {
    if bytes.len() > 4 * 1024 * 1024 {
        return fail(0, "tuning file exceeds limit");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| crate::Error {
        offset: 0,
        message: "invalid tuning UTF-8",
    })?;
    if text.bytes().any(|b| b == b'"' || b == 0) {
        return fail(0, "quoted or NUL tuning variant unsupported");
    }
    let mut out = Table {
        rows: Vec::new(),
        blank_lines: 0,
    };
    let mut count = 0;
    for line in text.lines() {
        if line.contains('\r') {
            return fail(0, "bare carriage return in tuning row");
        }
        if line.is_empty() {
            out.blank_lines += 1;
            continue;
        }
        let fields: Vec<_> = line.split(',').collect();
        count += fields.len();
        if fields.len() > 16384 || count > 1_000_000 || out.rows.len() >= 100_000 {
            return fail(0, "tuning row/field count limit");
        }
        out.rows.push(Row { fields });
    }
    if out.rows.is_empty() {
        return fail(0, "tuning table has no populated rows");
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_positional_strings_numeric_lexemes_empty_tail_and_blank_lines() {
        let t = read(b"car,Standard,1.140e3,-0.260000,\r\n\r\nroad,2,\n").unwrap();
        assert_eq!(t.blank_lines, 1);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(
            t.rows[0].fields,
            ["car", "Standard", "1.140e3", "-0.260000", ""]
        );
        assert_eq!(t.rows[0].number(2).unwrap(), Some(1140.));
        assert_eq!(t.rows[0].number(4).unwrap(), None);
        assert!(t.rows[0].number(0).is_err());
        assert!(t.rows[0].number(5).is_err());
    }
    #[test]
    fn rejects_unsupported_framing_and_nonfinite_numeric_requests() {
        for b in [b"".as_slice(), b"a,\"b,c\"", b"a\rb", b"a,\0", b"\xff"] {
            assert!(read(b).is_err());
        }
        let t = read(b"NaN,inf,1e999").unwrap();
        for i in 0..3 {
            assert!(t.rows[0].number(i).is_err());
        }
    }
}
