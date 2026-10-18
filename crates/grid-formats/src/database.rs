//! GRID NDBS database (`database/database.bin` with `database/schema.bin`):
//! every table decoded into typed columns. Same framing as
//! [`crate::livery_numbers`] (table markers `<i>LBT`, row markers `ITM<i>`,
//! strings padded to `((size + 5) / 4) * 4` bytes); the whole file is consumed.
use crate::{Error, Result, binxml, fail};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
    Bool(bool),
    Text(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Int,
    Float,
    Bool,
    Text(usize),
}

#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// One row of a table, read by column name.
#[derive(Clone, Copy)]
pub struct Row<'a> {
    table: &'a Table,
    values: &'a [Value],
}

impl Table {
    fn column(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }
    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.rows.iter().map(move |values| Row {
            table: self,
            values,
        })
    }
}

impl<'a> Row<'a> {
    pub fn get(&self, column: &str) -> Option<&'a Value> {
        self.table.column(column).map(|i| &self.values[i])
    }
    /// An int or bool column.
    pub fn int(&self, column: &str) -> Option<i32> {
        match self.get(column)? {
            Value::Int(v) => Some(*v),
            Value::Bool(v) => Some(*v as i32),
            _ => None,
        }
    }
    pub fn float(&self, column: &str) -> Option<f32> {
        match self.get(column)? {
            Value::Float(v) => Some(*v),
            _ => None,
        }
    }
    pub fn text(&self, column: &str) -> Option<&'a str> {
        match self.get(column)? {
            Value::Text(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Database {
    pub tables: Vec<Table>,
}

impl Database {
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }
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

fn width(kind: Kind) -> usize {
    match kind {
        Kind::Int | Kind::Float | Kind::Bool => 4,
        // Observed GRID storage reserves two bytes then aligns to 4.
        Kind::Text(size) => (size + 5) / 4 * 4,
    }
}

fn text(bytes: &[u8], size: usize, offset: usize) -> Result<String> {
    let end = bytes.iter().position(|&b| b == 0).ok_or(Error {
        offset,
        message: "database string lacks terminator",
    })?;
    if end > size || bytes[end..].iter().any(|&b| b != 0) {
        return fail(offset, "database string padding/size differs");
    }
    String::from_utf8(bytes[..end].to_vec()).map_err(|_| Error {
        offset,
        message: "database string is not UTF8",
    })
}

/// A table name and its (column, kind) list.
type TableSchema = (String, Vec<(String, Kind)>);

fn schema(bytes: &[u8]) -> Result<Vec<TableSchema>> {
    let doc = binxml::read(bytes)?;
    if doc.elements.first().map(|e| e.name) != Some("schema") {
        return fail(0, "database schema root differs");
    }
    let mut tables = Vec::new();
    for index in doc.elements[0].children.clone() {
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
            let kind = match attr(field, "type")? {
                "int" => Kind::Int,
                "float" => Kind::Float,
                "bool" => Kind::Bool,
                "string" => {
                    let size = attr(field, "size")?.parse::<usize>().map_err(|_| Error {
                        offset: f,
                        message: "invalid database string size",
                    })?;
                    if size == 0 || size > 4096 {
                        return fail(f, "database string size exceeds bound");
                    }
                    Kind::Text(size)
                }
                _ => return fail(f, "unknown database field type"),
            };
            columns.push((attr(field, "name")?.to_string(), kind));
        }
        tables.push((attr(e, "name")?.to_string(), columns));
    }
    if tables.is_empty() || tables.len() > 256 {
        return fail(0, "unsupported database table count");
    }
    Ok(tables)
}

/// Decodes every table of the database. Fails on any marker, padding or
/// length mismatch.
pub fn read(schema_bytes: &[u8], data: &[u8]) -> Result<Database> {
    let tables = schema(schema_bytes)?;
    if data.len() > 64 * 1024 * 1024 || data.get(..4) != Some(b"SBDN") {
        return fail(0, "unsupported database framing/bounds");
    }
    let mut offset = 4;
    let mut out = Vec::with_capacity(tables.len());
    for (index, (name, columns)) in tables.into_iter().enumerate() {
        let tag = [index as u8, b'L', b'B', b'T'];
        if data.get(offset..offset + 4) != Some(tag.as_slice()) {
            return fail(offset, "database table marker differs from schema order");
        }
        let count_bytes = data.get(offset + 4..offset + 8).ok_or(Error {
            offset,
            message: "truncated database table count",
        })?;
        let count = u32::from_le_bytes(count_bytes.try_into().unwrap()) as usize;
        let size: usize = columns.iter().map(|(_, k)| width(*k)).sum();
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
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            let marker = [b'I', b'T', b'M', index as u8];
            if data.get(offset..offset + 4) != Some(marker.as_slice()) {
                return fail(offset, "database row marker differs from table");
            }
            offset += 4;
            let mut row = Vec::with_capacity(columns.len());
            for (_, kind) in &columns {
                let w = width(*kind);
                let bytes = &data[offset..offset + w];
                let word = || u32::from_le_bytes(bytes[..4].try_into().unwrap());
                row.push(match kind {
                    Kind::Int => Value::Int(word() as i32),
                    Kind::Float => Value::Float(f32::from_bits(word())),
                    Kind::Bool => Value::Bool(word() != 0),
                    Kind::Text(size) => Value::Text(text(bytes, *size, offset)?),
                });
                offset += w;
            }
            rows.push(row);
        }
        out.push(Table {
            name,
            columns: columns.into_iter().map(|(n, _)| n).collect(),
            rows,
        });
    }
    if offset != data.len() {
        return fail(offset, "unexpected database tail or schema mismatch");
    }
    Ok(Database { tables: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_widths_reserve_two_bytes_and_align() {
        assert_eq!(width(Kind::Text(1)), 4);
        assert_eq!(width(Kind::Text(2)), 4);
        assert_eq!(width(Kind::Text(3)), 8);
        assert_eq!(width(Kind::Text(12)), 16);
        assert_eq!(width(Kind::Text(16)), 20);
        assert_eq!(width(Kind::Int), 4);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_database_decodes_every_table() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = read(&schema, &data).unwrap();
        assert_eq!(db.tables.len(), 78);
        let vehicle = db.table("vehicle").unwrap();
        assert_eq!(vehicle.rows.len(), 52);
        let lac = vehicle
            .rows()
            .find(|r| r.text("file_string") == Some("lac"))
            .unwrap();
        assert_eq!(lac.int("id"), Some(10));
        assert_eq!(lac.text("ai_vehicle_type_config"), Some("touring"));
        assert_eq!(lac.int("vehicle_drive_type_id"), Some(2));
        let pools = db.table("free_race_track_pool").unwrap();
        assert_eq!(pools.rows.len(), 766);
    }
}
