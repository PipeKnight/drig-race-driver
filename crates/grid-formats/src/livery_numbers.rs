//! GRID NDBS fixed-schema framing, selecting only vehicle_livery number text.
//! Other row values are skipped, not interpreted as recovered game behaviour.
use crate::{Error, Result, binxml, car_decals::Number, fail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveryNumber<'a> {
    pub file: &'a str,
    pub number: Option<Number>,
}
#[derive(Clone, Copy)]
struct Column<'a> {
    name: &'a str,
    width: usize,
    text_size: Option<usize>,
}
struct Table<'a> {
    name: &'a str,
    columns: Vec<Column<'a>>,
}
fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Result<&'a str> {
    let mut found = e.attributes.iter().filter(|(k, _)| *k == name);
    let value = found.next().map(|(_, v)| *v).ok_or(Error {
        offset: 0,
        message: "database schema field absent",
    })?;
    if found.next().is_some() {
        return fail(0, "duplicate database schema field");
    }
    Ok(value)
}
fn string(bytes: &[u8], size: usize) -> Result<&str> {
    let end = bytes.iter().position(|&b| b == 0).ok_or(Error {
        offset: 0,
        message: "database string lacks terminator",
    })?;
    if end > size || bytes[end..].iter().any(|&b| b != 0) {
        return fail(0, "database string padding/size differs");
    }
    std::str::from_utf8(&bytes[..end]).map_err(|_| Error {
        offset: 0,
        message: "database string is not UTF8",
    })
}
fn rows<'a>(tables: &[Table<'_>], data: &'a [u8]) -> Result<Vec<LiveryNumber<'a>>> {
    if data.len() > 64 * 1024 * 1024
        || data.get(..4) != Some(b"SBDN")
        || tables.is_empty()
        || tables.len() > 256
    {
        return fail(0, "unsupported database framing/bounds");
    }
    let mut offset = 4;
    let mut result = None;
    for (index, table) in tables.iter().enumerate() {
        let tag = [index as u8, b'L', b'B', b'T'];
        if data.get(offset..offset + 4) != Some(tag.as_slice()) {
            return fail(offset, "database table marker differs from schema order");
        }
        let count_bytes = data.get(offset + 4..offset + 8).ok_or(Error {
            offset,
            message: "truncated database table count",
        })?;
        let count = u32::from_le_bytes(count_bytes.try_into().unwrap()) as usize;
        let size: usize = table.columns.iter().map(|c| c.width).sum();
        if count > 100000 || size == 0 || size > 1024 * 1024 {
            return fail(offset, "database row bounds exceeded");
        }
        offset += 8;
        let needed = count.checked_mul(size + 4).ok_or(Error {
            offset,
            message: "database row size overflow",
        })?;
        if needed > data.len() - offset {
            return fail(offset, "truncated database rows");
        }
        let selected = table.name == "vehicle_livery";
        if selected && result.is_some() {
            return fail(offset, "duplicate vehicle_livery table");
        }
        let mut numbers = Vec::new();
        for _ in 0..count {
            let marker = [b'I', b'T', b'M', index as u8];
            if data.get(offset..offset + 4) != Some(marker.as_slice()) {
                return fail(offset, "database row marker differs from table");
            }
            offset += 4;
            let mut file = None;
            let mut number = None;
            for col in &table.columns {
                let bytes = &data[offset..offset + col.width];
                if selected && matches!(col.name, "file_string" | "number_on_livery") {
                    let text = string(
                        bytes,
                        col.text_size.ok_or(Error {
                            offset,
                            message: "livery number columns must be strings",
                        })?,
                    )?;
                    match col.name {
                        "file_string" if file.is_none() => file = Some(text),
                        "number_on_livery" if number.is_none() => {
                            number = Some(if matches!(text, "" | "-1") {
                                None
                            } else {
                                Some(Number::parse(text)?)
                            })
                        }
                        _ => return fail(offset, "duplicate livery number column"),
                    }
                }
                offset += col.width;
            }
            if selected {
                numbers.push(LiveryNumber {
                    file: file.ok_or(Error {
                        offset,
                        message: "livery file column absent",
                    })?,
                    number: number.ok_or(Error {
                        offset,
                        message: "livery number column absent",
                    })?,
                });
            }
        }
        if selected {
            result = Some(numbers);
        }
    }
    if offset != data.len() {
        return fail(offset, "unexpected database tail or schema mismatch");
    }
    result.ok_or(Error {
        offset,
        message: "vehicle_livery table absent",
    })
}
/// The shipped schema and DB are consumed completely, with every table/row marker checked.
pub fn read<'a>(schema: &[u8], data: &'a [u8]) -> Result<Vec<LiveryNumber<'a>>> {
    let doc = binxml::read(schema)?;
    if doc.elements.first().map(|e| e.name) != Some("schema") {
        return fail(0, "database schema root differs");
    }
    let root = &doc.elements[0];
    let mut tables = Vec::new();
    for index in root.children.clone() {
        let e = &doc.elements[index];
        if e.name != "table" || e.children.len() > 256 {
            return fail(index, "unsupported database schema table");
        }
        let mut columns = Vec::new();
        for f in e.children.clone() {
            let field = &doc.elements[f];
            if field.name != "field" {
                return fail(f, "unsupported database schema column");
            }
            let (width, text_size) = match attr(field, "type")? {
                "int" | "float" | "bool" => (4, None),
                "string" => {
                    let size = attr(field, "size")?.parse::<usize>().map_err(|_| Error {
                        offset: f,
                        message: "invalid database string size",
                    })?;
                    if size == 0 || size > 4096 {
                        return fail(f, "database string size exceeds bound");
                    }
                    // Observed GRID storage reserves two bytes then aligns to4.
                    // All78 source tables/row markers/end-of-file verify this layout.
                    (((size + 5) / 4) * 4, Some(size))
                }
                _ => return fail(f, "unknown database field type"),
            };
            columns.push(Column {
                name: attr(field, "name")?,
                width,
                text_size,
            });
        }
        tables.push(Table {
            name: attr(e, "name")?,
            columns,
        });
    }
    rows(&tables, data)
}
/// Duplicate file aliases are allowed only when they select the same number.
pub fn select(rows: &[LiveryNumber<'_>], file: &str) -> Result<Option<Number>> {
    let mut matches = rows.iter().filter(|r| r.file == file);
    let first = matches.next().ok_or(Error {
        offset: 0,
        message: "livery number selection absent",
    })?;
    if matches.any(|r| r.number != first.number) {
        return fail(0, "ambiguous livery number selection");
    }
    Ok(first.number)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_preserves_zeroes_and_rejects_bad_markers_or_tail() {
        let tables = [Table {
            name: "vehicle_livery",
            columns: vec![
                Column {
                    name: "file_string",
                    width: 8,
                    text_size: Some(6),
                },
                Column {
                    name: "number_on_livery",
                    width: 8,
                    text_size: Some(4),
                },
            ],
        }];
        let mut b = b"SBDN\0LBT\x01\0\0\0ITM\0lac_00\0\0".to_vec();
        b.extend_from_slice(b"01\0\0\0\0\0\0");
        let result = rows(&tables, &b).unwrap();
        assert_eq!(select(&result, "lac_00").unwrap().unwrap().as_str(), "01");
        for at in [0, 4, 12] {
            let mut invalid = b.clone();
            invalid[at] ^= 1;
            assert!(rows(&tables, &invalid).is_err());
        }
        assert!(rows(&tables, &b[..b.len() - 1]).is_err());
        b.push(0);
        assert!(rows(&tables, &b).is_err());
    }
    #[test]
    fn disabled_and_ambiguous_selections_are_distinct_from_missing() {
        let n = Number::parse("01").unwrap();
        let mut r = vec![LiveryNumber {
            file: "x",
            number: None,
        }];
        assert_eq!(select(&r, "x").unwrap(), None);
        assert!(select(&r, "missing").is_err());
        r.push(LiveryNumber {
            file: "x",
            number: Some(n),
        });
        assert!(select(&r, "x").is_err());
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_database_selects_lacetti_number_with_leading_zero() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let r = read(&schema, &data).unwrap();
        assert!(r.len() > 300);
        assert_eq!(select(&r, "lac_00").unwrap().unwrap().as_str(), "01");
        assert_eq!(select(&r, "lac_01").unwrap().unwrap().as_str(), "25");
        assert_eq!(select(&r, "tra_user").unwrap(), None);
    }
}
