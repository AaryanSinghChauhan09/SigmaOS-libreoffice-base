/// Database frontend component for SigmaOS (LibreOffice Base equivalent).

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    Integer,
    Text,
    Real,
    Boolean,
    Blob,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
    pub nullable: bool,
}

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug)]
pub struct Database {
    pub name: String,
    tables: Vec<TableSchema>,
}

impl Database {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tables: Vec::new(),
        }
    }

    pub fn create_table(&mut self, schema: TableSchema) -> Result<(), String> {
        if self.tables.iter().any(|t| t.name == schema.name) {
            return Err(format!("Table '{}' already exists", schema.name));
        }
        self.tables.push(schema);
        Ok(())
    }

    pub fn drop_table(&mut self, name: &str) -> Result<(), String> {
        let pos = self
            .tables
            .iter()
            .position(|t| t.name == name)
            .ok_or_else(|| format!("Table '{}' not found", name))?;
        self.tables.remove(pos);
        Ok(())
    }

    pub fn get_table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|t| t.name == name)
    }

    pub fn list_tables(&self) -> Vec<&str> {
        self.tables.iter().map(|t| t.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema(name: &str) -> TableSchema {
        TableSchema {
            name: name.to_string(),
            columns: vec![ColumnDef {
                name: "id".to_string(),
                col_type: ColumnType::Integer,
                nullable: false,
            }],
        }
    }

    #[test]
    fn test_create_table_aaa() {
        // Arrange
        let mut db = Database::new("testdb");
        let schema = make_schema("users");

        // Act
        let result = db.create_table(schema);

        // Assert
        assert!(result.is_ok());
        assert!(db.get_table("users").is_some());
    }

    #[test]
    fn test_drop_table_aaa() {
        // Arrange
        let mut db = Database::new("testdb");
        db.create_table(make_schema("orders")).unwrap();

        // Act
        let result = db.drop_table("orders");

        // Assert
        assert!(result.is_ok());
        assert!(db.get_table("orders").is_none());
    }

    #[test]
    fn test_list_tables_aaa() {
        // Arrange
        let mut db = Database::new("testdb");
        db.create_table(make_schema("alpha")).unwrap();
        db.create_table(make_schema("beta")).unwrap();

        // Act
        let tables = db.list_tables();

        // Assert
        assert_eq!(tables.len(), 2);
        assert!(tables.contains(&"alpha"));
        assert!(tables.contains(&"beta"));
    }
}
