
#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub models: Vec<Model>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub name: String,
    pub fields: Vec<Field>,
    pub attributes: Vec<ModelAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub nullable: bool,
    pub is_array: bool,
    pub attributes: Vec<FieldAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    Int,
    String,
    Boolean,
    Float,
    DateTime,
    Decimal,
    Json,
    Model(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldAttribute {
    Id,
    Unique,
    Default(DefaultValue),
    Map(String),
    UpdatedAt,
    Relation(RelationAttribute),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DefaultValue {
    AutoIncrement,
    Now,
    Cuid,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Identifier(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelationAttribute {
    pub name: Option<String>,
    pub fields: Vec<String>,
    pub references: Vec<String>,
    pub through: Option<String>,
    pub pivot_from: Option<String>,
    pub pivot_to: Option<String>,
    pub on_delete: Option<ReferentialAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReferentialAction {
    Cascade,
    Restrict,
    SetNull,
    NoAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelAttribute {
    Map(String),
    Index(Vec<String>),
    Unique(Vec<String>),
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_ast_can_represent_nullable_and_array_fields() {
        let description = Field {
            name: "description".to_string(),
            field_type: FieldType::String,
            nullable: true,
            is_array: false,
            attributes: vec![],
        };

        let tags = Field {
            name: "tags".to_string(),
            field_type: FieldType::String,
            nullable: false,
            is_array: true,
            attributes: vec![],
        };

        assert!(description.nullable);
        assert!(!description.is_array);

        assert!(!tags.nullable);
        assert!(tags.is_array);
    }
}