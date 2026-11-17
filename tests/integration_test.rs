use sigmaos_libreoffice_base::{ColumnDef, ColumnType, Database, TableSchema};

#[test]
fn test_create_table_aaa() {
    // Arrange
    let mut db = Database::new("integration_db");
    let schema = TableSchema {
        name: "products".to_string(),
        columns: vec![
            ColumnDef {
                name: "id".to_string(),
                col_type: ColumnType::Integer,
                nullable: false,
                primary_key: false,
            },
            ColumnDef {
                name: "name".to_string(),
                col_type: ColumnType::Text,
                nullable: false,
                primary_key: false,
            },
            ColumnDef {
                name: "price".to_string(),
                col_type: ColumnType::Real,
                nullable: true,
                primary_key: false,
            },
        ],
    };

    // Act
    let result = db.create_table(schema);

    // Assert
    assert!(result.is_ok());
    let t = db.get_table("products").unwrap();
    assert_eq!(t.columns.len(), 3);
}

#[test]
fn test_duplicate_table_rejected_aaa() {
    // Arrange
    let mut db = Database::new("integration_db");
    let schema = TableSchema {
        name: "users".to_string(),
        columns: vec![],
    };
    db.create_table(schema.clone()).unwrap();

    // Act
    let result = db.create_table(schema);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_drop_missing_table_aaa() {
    // Arrange
    let mut db = Database::new("integration_db");

    // Act
    let result = db.drop_table("nonexistent");

    // Assert
    assert!(result.is_err());
}
