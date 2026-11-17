use sigmaos_libreoffice_base::{BaseError, ColumnDef, ColumnType, Database, Row, TableSchema, Value};
use std::path::Path;

fn usage() -> ! {
    eprintln!("usage:");
    eprintln!("  sigmaos-libreoffice-base new FILE DBNAME");
    eprintln!("  sigmaos-libreoffice-base table FILE TABLE (COLUMN TYPE [NULLABLE] [PK])*");
    eprintln!("  sigmaos-libreoffice-base insert FILE TABLE (VAL)*");
    eprintln!("  sigmaos-libreoffice-base select FILE TABLE");
    eprintln!("  sigmaos-libreoffice-base delete FILE TABLE");
    eprintln!("  sigmaos-libreoffice-base drop FILE TABLE");
    eprintln!("  sigmaos-libreoffice-base csv FILE TABLE OUT.csv");
    eprintln!("  sigmaos-libreoffice-base info FILE");
    eprintln!("VALUES: N INT REAL BOOL TEXT|STR BLOB|HEX");
    eprintln!("TYPES: INTEGER TEXT REAL BOOLEAN BLOB");
    eprintln!("FLAGS: NULL|NOTNULL PK");
    std::process::exit(2);
}

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("error: {e}");
    std::process::exit(1);
}

fn parse_type(s: &str) -> ColumnType {
    ColumnType::parse(s).unwrap_or_else(|_| fail(format!("unknown type: {s}")))
}

fn parse_value(s: &str) -> Value {
    match s {
        "N" | "NULL" => Value::Null,
        "INT" => Value::Integer(0),
        v if v.starts_with("INT:") => v[4..].parse::<i64>().map(Value::Integer).unwrap_or_else(|_| fail("bad int")),
        "REAL" => Value::Real(0.0),
        v if v.starts_with("REAL:") => v[5..].parse::<f64>().map(Value::Real).unwrap_or_else(|_| fail("bad real")),
        "BOOL" => Value::Boolean(false),
        v if v.starts_with("BOOL:") => match &v[5..] {
            "true" | "1" => Value::Boolean(true),
            "false" | "0" => Value::Boolean(false),
            _ => fail("bad bool"),
        },
        v if v.starts_with("TEXT:") || v.starts_with("STR:") => Value::Text(v[5..].to_string()),
        v if v.starts_with("BLOB:") => hex::decode(&v[5..]).map(Value::Blob).unwrap_or_else(|_| fail("bad hex")),
        other => Value::Text(other.to_string()),
    }
}

fn load(file: &str) -> Database {
    if !Path::new(file).exists() {
        fail(format!("file not found: {file}"));
    }
    Database::load(file).unwrap_or_else(|e| fail(e))
}

fn save(db: &Database, file: &str) {
    db.save(file).unwrap_or_else(|e: BaseError| fail(e));
}

mod hex {
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        usage();
    }
    let cmd = args[0].as_str();
    let file = args[1].as_str();

    match cmd {
        "new" => {
            let name = args[2..].join(" ");
            let db = Database::new(&name);
            save(&db, file);
            println!("created {file}");
        }
        "table" => {
            if args.len() < 4 {
                usage();
            }
            let table = args[2].as_str();
            let mut schema = TableSchema::new(table);
            let mut i = 3;
            while i + 1 < args.len() {
                let name = args[i].as_str();
                let col_type = parse_type(&args[i + 1]);
                let nullable = i + 2 < args.len() && args[i + 2] != "NOTNULL";
                let pk = i + 2 < args.len() && args[i + 2] == "PK";
                let col = ColumnDef::new(name, col_type).nullable(nullable);
                let col = if pk { col.primary_key() } else { col };
                schema = schema.add_column(col);
                i += if pk || (i + 2 < args.len() && args[i + 2] == "NOTNULL") { 3 } else { 2 };
            }
            let mut db = load(file);
            db.create_table(schema).unwrap_or_else(|e| fail(e));
            save(&db, file);
            println!("created table {table}");
        }
        "insert" => {
            if args.len() < 4 {
                usage();
            }
            let table = args[2].as_str();
            let values: Vec<Value> = args[3..].iter().map(|s| parse_value(s)).collect();
            let mut db = load(file);
            db.insert(table, Row::new(values)).unwrap_or_else(|e| fail(e));
            save(&db, file);
            println!("inserted into {table}");
        }
        "select" => {
            if args.len() < 3 {
                usage();
            }
            let table = args[2].as_str();
            let db = load(file);
            let rows = db.select_where(table, |_| true).unwrap_or_else(|e| fail(e));
            println!("{} rows in {}", rows.len(), table);
            for r in rows {
                println!("  {}", r.values.iter().map(|v| v.as_text()).collect::<Vec<_>>().join(" | "));
            }
        }
        "delete" => {
            if args.len() < 3 {
                usage();
            }
            let table = args[2].as_str();
            let mut db = load(file);
            let deleted = db.delete_where(table, |_| true).unwrap_or_else(|e| fail(e));
            save(&db, file);
            println!("deleted {deleted} rows from {table}");
        }
        "drop" => {
            if args.len() < 3 {
                usage();
            }
            let table = args[2].as_str();
            let mut db = load(file);
            db.drop_table(table).unwrap_or_else(|e| fail(e));
            save(&db, file);
            println!("dropped table {table}");
        }
        "csv" => {
            if args.len() < 4 {
                usage();
            }
            let table = args[2].as_str();
            let out = args[3].as_str();
            let db = load(file);
            let csv = db.export_csv(table).unwrap_or_else(|e| fail(e));
            std::fs::write(out, csv).unwrap_or_else(|e| fail(e));
            println!("exported {out}");
        }
        "info" => {
            let db = load(file);
            println!("{} ({} tables)", db.name, db.list_tables().len());
            for t in db.list_tables() {
                if let Some(schema) = db.get_table(t) {
                    println!("  table {}", t);
                    for c in &schema.columns {
                        println!("    {} {}{}{}", c.name, c.col_type.as_str(), if c.nullable { "" } else { " NOT NULL" }, if c.primary_key { " PK" } else { "" });
                    }
                }
            }
        }
        _ => usage(),
    }
}
