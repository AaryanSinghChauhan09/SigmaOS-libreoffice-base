//! In-memory relational database with schema, rows, basic queries, and CSV export.
//!
//! Supports INTEGER, TEXT, REAL, BOOLEAN, BLOB columns with nullable flags,
//! primary key simulation, row CRUD, simple WHERE selection, and persistence.

use std::fmt;
use std::fs;
use std::path::Path;

/// Column data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Integer,
    Text,
    Real,
    Boolean,
    Blob,
}

impl ColumnType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "INTEGER",
            Self::Text => "TEXT",
            Self::Real => "REAL",
            Self::Boolean => "BOOLEAN",
            Self::Blob => "BLOB",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_uppercase().as_str() {
            "INTEGER" => Ok(Self::Integer),
            "TEXT" => Ok(Self::Text),
            "REAL" => Ok(Self::Real),
            "BOOLEAN" => Ok(Self::Boolean),
            "BLOB" => Ok(Self::Blob),
            other => Err(format!("unknown type: {other}")),
        }
    }
}

/// Column definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
    pub nullable: bool,
    pub primary_key: bool,
}

impl ColumnDef {
    pub fn new(name: impl Into<String>, col_type: ColumnType) -> Self {
        Self { name: name.into(), col_type, nullable: true, primary_key: false }
    }

    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    pub fn primary_key(mut self) -> Self {
        self.primary_key = true;
        self.nullable = false;
        self
    }
}

/// A single cell value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Text(String),
    Real(f64),
    Boolean(bool),
    Blob(Vec<u8>),
}

impl Value {
    pub fn type_of(&self) -> ColumnType {
        match self {
            Self::Null => ColumnType::Text,
            Self::Integer(_) => ColumnType::Integer,
            Self::Text(_) => ColumnType::Text,
            Self::Real(_) => ColumnType::Real,
            Self::Boolean(_) => ColumnType::Boolean,
            Self::Blob(_) => ColumnType::Blob,
        }
    }

    pub fn as_text(&self) -> String {
        match self {
            Self::Null => "NULL".to_string(),
            Self::Integer(n) => n.to_string(),
            Self::Text(s) => s.clone(),
            Self::Real(f) => f.to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::Blob(data) => format!("<blob {} bytes>", data.len()),
        }
    }
}

/// Table schema with column definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

impl TableSchema {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), columns: Vec::new() }
    }

    pub fn add_column(mut self, col: ColumnDef) -> Self {
        self.columns.push(col);
        self
    }

    pub fn primary_key_index(&self) -> Option<usize> {
        self.columns.iter().position(|c| c.primary_key)
    }
}

/// A row of values.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }
}

/// Errors for database operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseError {
    TableNotFound(String),
    ColumnNotFound(String),
    DuplicateTable(String),
    DuplicateRow,
    InvalidColumnCount { expected: usize, actual: usize },
    TypeMismatch(String),
    NullViolation(String),
    Io(String),
    Format(String),
}

impl fmt::Display for BaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TableNotFound(t) => write!(f, "table not found: {t}"),
            Self::ColumnNotFound(c) => write!(f, "column not found: {c}"),
            Self::DuplicateTable(t) => write!(f, "table already exists: {t}"),
            Self::DuplicateRow => write!(f, "duplicate primary key"),
            Self::InvalidColumnCount { expected, actual } => write!(f, "column count mismatch: expected {expected}, got {actual}"),
            Self::TypeMismatch(msg) => write!(f, "type mismatch: {msg}"),
            Self::NullViolation(col) => write!(f, "null violation on column: {col}"),
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Format(e) => write!(f, "format error: {e}"),
        }
    }
}

impl std::error::Error for BaseError {}

const MAGIC: &str = "SIGBASE1";

/// In-memory relational database.
#[derive(Debug, Clone, PartialEq)]
pub struct Database {
    pub name: String,
    tables: Vec<TableSchema>,
    data: std::collections::HashMap<String, Vec<Row>>,
}

impl Database {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), tables: Vec::new(), data: std::collections::HashMap::new() }
    }

    pub fn create_table(&mut self, schema: TableSchema) -> Result<(), BaseError> {
        if self.tables.iter().any(|t| t.name == schema.name) {
            return Err(BaseError::DuplicateTable(schema.name));
        }
        let name = schema.name.clone();
        self.tables.push(schema);
        self.data.insert(name, Vec::new());
        Ok(())
    }

    pub fn drop_table(&mut self, name: &str) -> Result<(), BaseError> {
        let pos = self.tables.iter().position(|t| t.name == name).ok_or_else(|| BaseError::TableNotFound(name.to_string()))?;
        self.tables.remove(pos);
        self.data.remove(name);
        Ok(())
    }

    pub fn get_table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|t| t.name == name)
    }

    pub fn list_tables(&self) -> Vec<&str> {
        self.tables.iter().map(|t| t.name.as_str()).collect()
    }

    fn rows_mut(&mut self, table: &str) -> Result<&mut Vec<Row>, BaseError> {
        self.data.get_mut(table).ok_or_else(|| BaseError::TableNotFound(table.to_string()))
    }

    fn rows(&self, table: &str) -> Result<&Vec<Row>, BaseError> {
        self.data.get(table).ok_or_else(|| BaseError::TableNotFound(table.to_string()))
    }

    fn schema(&self, table: &str) -> Result<&TableSchema, BaseError> {
        self.get_table(table).ok_or_else(|| BaseError::TableNotFound(table.to_string()))
    }

    /// Insert a row into a table, enforcing column count and nullability.
    pub fn insert(&mut self, table: &str, row: Row) -> Result<(), BaseError> {
        let schema = self.schema(table)?.clone();
        if row.values.len() != schema.columns.len() {
            return Err(BaseError::InvalidColumnCount { expected: schema.columns.len(), actual: row.values.len() });
        }
        for (i, val) in row.values.iter().enumerate() {
            let col = &schema.columns[i];
            if !col.nullable && val == &Value::Null {
                return Err(BaseError::NullViolation(col.name.clone()));
            }
        }
        let rows = self.rows_mut(table)?;
        if let Some(pk_idx) = schema.primary_key_index() {
            let pk_val = &row.values[pk_idx];
            if rows.iter().any(|r| r.values[pk_idx] == *pk_val) {
                return Err(BaseError::DuplicateRow);
            }
        }
        rows.push(row);
        Ok(())
    }

    /// Delete rows matching a predicate.
    pub fn delete_where(&mut self, table: &str, predicate: impl Fn(&Row) -> bool) -> Result<usize, BaseError> {
        let rows = self.rows_mut(table)?;
        let before = rows.len();
        rows.retain(|r| !predicate(r));
        Ok(before - rows.len())
    }

    /// Select rows matching a predicate.
    pub fn select_where(&self, table: &str, predicate: impl Fn(&Row) -> bool) -> Result<Vec<Row>, BaseError> {
        let rows = self.rows(table)?;
        Ok(rows.iter().filter(|r| predicate(r)).cloned().collect())
    }

    /// Update rows matching a predicate with a transformation.
    pub fn update_where(&mut self, table: &str, predicate: impl Fn(&Row) -> bool, transform: impl Fn(&mut Row)) -> Result<usize, BaseError> {
        let rows = self.rows_mut(table)?;
        let mut count = 0;
        for r in rows.iter_mut() {
            if predicate(r) {
                transform(r);
                count += 1;
            }
        }
        Ok(count)
    }

    /// Export a table to CSV.
    pub fn export_csv(&self, table: &str) -> Result<String, BaseError> {
        let schema = self.schema(table)?.clone();
        let rows = self.rows(table)?.clone();
        let mut out = String::new();
        out.push_str(&schema.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(","));
        out.push('\n');
        for r in rows {
            out.push_str(&r.values.iter().map(|v| csv_value(v)).collect::<Vec<_>>().join(","));
            out.push('\n');
        }
        Ok(out)
    }

    /// Serialize to the line-based persistence format.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), BaseError> {
        let mut out = String::new();
        out.push_str(MAGIC);
        out.push('\n');
        out.push_str(&format!("NAME|{}\n", encode_field(&self.name)));
        for schema in &self.tables {
            out.push_str(&format!("TABLE|{}\n", encode_field(&schema.name)));
            for col in &schema.columns {
                out.push_str(&format!(
                    "COLUMN|{}|{}|{}|{}\n",
                    encode_field(&col.name),
                    col.col_type.as_str(),
                    col.nullable as u8,
                    col.primary_key as u8
                ));
            }
            out.push_str("ENDCOLUMN\n");
            if let Some(rows) = self.data.get(&schema.name) {
                for r in rows {
                    out.push_str(&format!("ROW|{}\n", r.values.iter().map(|v| encode_value(v)).collect::<Vec<_>>().join("|")));
                }
            }
            out.push_str("ENDTABLE\n");
        }
        fs::write(path, out).map_err(|e| BaseError::Io(e.to_string()))
    }

    /// Load a database saved with [`Database::save`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self, BaseError> {
        let text = fs::read_to_string(path).map_err(|e| BaseError::Io(e.to_string()))?;
        let mut lines = text.lines();
        if lines.next() != Some(MAGIC) {
            return Err(BaseError::Format("bad magic".to_string()));
        }
        let mut db = Database::new("");
        let mut name = String::new();
        let mut current_table: Option<String> = None;
        for line in lines {
            if let Some(rest) = line.strip_prefix("NAME|") {
                name = decode_field(rest);
                db.name = name.clone();
            } else if let Some(rest) = line.strip_prefix("TABLE|") {
                let tname = decode_field(rest);
                current_table = Some(tname.clone());
                db.create_table(TableSchema::new(tname)).unwrap();
            } else if let Some(rest) = line.strip_prefix("COLUMN|") {
                let mut parts = rest.split('|');
                let cname = decode_field(parts.next().unwrap_or(""));
                let ctype = ColumnType::parse(parts.next().unwrap_or("TEXT")).map_err(BaseError::Format)?;
                let nullable = parts.next().unwrap_or("1") == "1";
                let pk = parts.next().unwrap_or("0") == "1";
                if let Some(ref tname) = current_table {
                    if let Some(s) = db.tables.iter_mut().find(|s| s.name == *tname) {
                        s.columns.push(ColumnDef { name: cname, col_type: ctype, nullable, primary_key: pk });
                    }
                }
            } else if line == "ENDCOLUMN" {
                continue;
            } else if let Some(rest) = line.strip_prefix("ROW|") {
                if let Some(ref tname) = current_table {
                    let tokens: Vec<&str> = rest.split('|').collect();
                    let mut values = Vec::new();
                    let mut i = 0;
                    while i + 1 < tokens.len() {
                        let typ = tokens[i];
                        let val = tokens[i + 1];
                        let v = match typ {
                            "NULL" => Value::Null,
                            "INT" => val.parse::<i64>().map(Value::Integer).map_err(|_| BaseError::Format("bad int".into()))?,
                            "REAL" => val.parse::<f64>().map(Value::Real).map_err(|_| BaseError::Format("bad real".into()))?,
                            "BOOL" => match val {
                                "true" | "1" => Value::Boolean(true),
                                "false" | "0" => Value::Boolean(false),
                                _ => return Err(BaseError::Format("bad bool".into())),
                            },
                            "TEXT" => Value::Text(decode_field(val)),
                            "BLOB" => hex::decode(val).map(Value::Blob).map_err(|_| BaseError::Format("bad hex".into()))?,
                            _ => return Err(BaseError::Format(format!("unknown type: {typ}"))),
                        };
                        values.push(v);
                        i += 2;
                    }
                    db.data.entry(tname.clone()).or_insert_with(Vec::new).push(Row::new(values));
                }
            } else if line == "ENDTABLE" {
                current_table = None;
            }
        }
        Ok(db)
    }
}

fn csv_value(v: &Value) -> String {
    match v {
        Value::Null => "".to_string(),
        Value::Text(s) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
        Value::Integer(n) => n.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Blob(_) => "<blob>".to_string(),
    }
}

fn encode_value(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Integer(n) => format!("INT|{n}"),
        Value::Text(s) => format!("TEXT|{}", encode_field(s)),
        Value::Real(f) => format!("REAL|{f}"),
        Value::Boolean(b) => format!("BOOL|{b}"),
        Value::Blob(data) => format!("BLOB|{}", hex::encode(data)),
    }
}

fn decode_value(s: &str) -> Result<Value, BaseError> {
    if s == "NULL" {
        return Ok(Value::Null);
    }
    if let Some(rest) = s.strip_prefix("TEXT|") {
        return Ok(Value::Text(decode_field(rest)));
    }
    if let Some(rest) = s.strip_prefix("INT|") {
        return rest.parse::<i64>().map(Value::Integer).map_err(|_| BaseError::Format("bad integer".into()));
    }
    if let Some(rest) = s.strip_prefix("REAL|") {
        return rest.parse::<f64>().map(Value::Real).map_err(|_| BaseError::Format("bad real".into()));
    }
    if let Some(rest) = s.strip_prefix("BOOL|") {
        return match rest {
            "true" | "1" => Ok(Value::Boolean(true)),
            "false" | "0" => Ok(Value::Boolean(false)),
            _ => Err(BaseError::Format("bad bool".into())),
        };
    }
    if let Some(rest) = s.strip_prefix("BLOB|") {
        return hex::decode(rest).map(Value::Blob).map_err(|_| BaseError::Format("bad hex".into()));
    }
    s.parse::<i64>().map(Value::Integer).map_err(|_| BaseError::Format("bad integer".into()))
}

fn encode_field(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\p").replace('\n', "\\n")
}

fn decode_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('p') => out.push('|'),
                Some('\\') => out.push('\\'),
                Some(other) => { out.push('\\'); out.push(other); }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("odd hex length".into());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema(name: &str) -> TableSchema {
        TableSchema::new(name)
            .add_column(ColumnDef::new("id", ColumnType::Integer).primary_key())
            .add_column(ColumnDef::new("name", ColumnType::Text).nullable(false))
            .add_column(ColumnDef::new("age", ColumnType::Real).nullable(true))
    }

    #[test]
    fn test_create_drop_list_aaa() {
        // Arrange
        let mut db = Database::new("test");

        // Act
        db.create_table(make_schema("users")).unwrap();
        let list: Vec<String> = db.list_tables().iter().map(|s| s.to_string()).collect();
        db.drop_table("users").unwrap();
        let after = db.list_tables();

        // Assert
        assert_eq!(list, vec!["users"]);
        assert!(after.is_empty());
        assert!(db.create_table(make_schema("users")).is_ok());
        assert!(db.create_table(make_schema("users")).is_err());
    }

    #[test]
    fn test_insert_select_delete_aaa() {
        // Arrange
        let mut db = Database::new("test");
        db.create_table(make_schema("users")).unwrap();

        // Act
        db.insert("users", Row::new(vec![Value::Integer(1), Value::Text("alice".into()), Value::Real(30.0)])).unwrap();
        db.insert("users", Row::new(vec![Value::Integer(2), Value::Text("bob".into()), Value::Null])).unwrap();
        let all = db.select_where("users", |_| true).unwrap();
        let adults = db.select_where("users", |r| {
            r.get(2).map(|v| matches!(v, Value::Real(a) if *a >= 18.0)).unwrap_or(false)
        }).unwrap();
        let deleted = db.delete_where("users", |r| r.get(1).map(|v| v == &Value::Text("bob".into())).unwrap_or(false)).unwrap();

        // Assert
        assert_eq!(all.len(), 2);
        assert_eq!(adults.len(), 1);
        assert_eq!(deleted, 1);
        assert_eq!(db.rows("users").unwrap().len(), 1);
    }

    #[test]
    fn test_constraints_aaa() {
        // Arrange
        let mut db = Database::new("test");
        db.create_table(make_schema("users")).unwrap();

        // Act / Assert
        assert!(db.insert("users", Row::new(vec![Value::Integer(1), Value::Null, Value::Null])).is_err());
        assert!(db.insert("users", Row::new(vec![Value::Integer(1), Value::Text("x".into()), Value::Null])).is_ok());
        assert!(db.insert("users", Row::new(vec![Value::Integer(1), Value::Text("y".into()), Value::Null])).is_err());
        assert!(db.insert("users", Row::new(vec![Value::Integer(2), Value::Text("y".into())])).is_err());
    }

    #[test]
    fn test_update_aaa() {
        // Arrange
        let mut db = Database::new("test");
        db.create_table(make_schema("users")).unwrap();
        db.insert("users", Row::new(vec![Value::Integer(1), Value::Text("alice".into()), Value::Real(30.0)])).unwrap();

        // Act
        let updated = db.update_where("users", |r| r.get(1).map(|v| v == &Value::Text("alice".into())).unwrap_or(false), |r| {
            r.values[2] = Value::Real(31.0);
        }).unwrap();

        // Assert
        assert_eq!(updated, 1);
        assert_eq!(db.rows("users").unwrap()[0].values[2], Value::Real(31.0));
    }

    #[test]
    fn test_csv_export_aaa() {
        // Arrange
        let mut db = Database::new("test");
        db.create_table(make_schema("users")).unwrap();
        db.insert("users", Row::new(vec![Value::Integer(1), Value::Text("a,b".into()), Value::Null])).unwrap();

        // Act
        let csv = db.export_csv("users").unwrap();

        // Assert
        assert!(csv.starts_with("id,name,age"));
        assert!(csv.contains("\"a,b\""));
    }

    #[test]
    fn test_save_load_roundtrip_aaa() {
        // Arrange
        let mut db = Database::new("round");
        db.create_table(make_schema("users")).unwrap();
        db.insert("users", Row::new(vec![Value::Integer(1), Value::Text("a|b\nc".into()), Value::Real(1.5)])).unwrap();
        let dir = std::env::temp_dir().join(format!("base-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("db.sigbase");

        // Act
        db.save(&path).unwrap();
        let loaded = Database::load(&path).unwrap();

        // Assert
        assert_eq!(loaded.name, "round");
        assert_eq!(loaded.list_tables(), vec!["users"]);
        assert_eq!(loaded.rows("users").unwrap().len(), 1);
        let row = &loaded.rows("users").unwrap()[0];
        assert_eq!(row.values[0], Value::Integer(1));
        assert_eq!(row.values[1], Value::Text("a|b\nc".into()));
        assert_eq!(row.values[2], Value::Real(1.5));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
