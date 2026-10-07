use crate::ast::{FieldType, Model, Schema};
use std::fs;
use std::path::Path;
pub fn generate(schema: &Schema) -> String {
    let mut output = String::new();

    output.push_str(
        "use rustorm::{\n\
         entity::{Column, Entity},\n\
         field::Field,\n\
         };\n\n",
    );

    for model in &schema.models {
        generate_model(model, &mut output);
    }

    output
}

fn generate_model(model: &Model, output: &mut String) {
    let model_struct = format!("{}Model", model.name);

    output.push_str("#[derive(Debug, sqlx::FromRow)]\n");
    output.push_str(&format!("pub struct {model_struct} {{\n"));

    for field in &model.fields {
        let rust_type = rust_type(&field.field_type, field.nullable, field.is_array);

        output.push_str(&format!(
            "    pub {}: {},\n",
            field.name, rust_type
        ));
    }

    output.push_str("}\n\n");

    output.push_str(&format!("pub struct {};\n\n", model.name));

    output.push_str(&format!("impl Entity for {} {{\n", model.name));
    output.push_str(&format!("    type Model = {model_struct};\n"));
    output.push_str(&format!(
        "    const TABLE: &'static str = \"{}\";\n",
        table_name(model)
    ));

    output.push_str("    const COLUMNS: &'static [Column] = &[\n");

    for field in &model.fields {
        output.push_str(&format!(
            "        Column::new(\"{}\"),\n",
            column_name(field)
        ));
    }

    output.push_str("    ];\n");
    output.push_str("}\n\n");

    output.push_str(&format!("impl {} {{\n", model.name));

    for field in &model.fields {
        let rust_type = rust_type(&field.field_type, field.nullable, field.is_array);

        output.push_str("    #[allow(non_upper_case_globals)]\n");
        output.push_str(&format!(
            "    pub const {}: Field<Self, {}> = Field::new(\"{}\");\n\n",
            field.name,
            rust_type,
            column_name(field)
        ));
    }

    output.push_str("}\n\n");
}

fn rust_type(field_type: &FieldType, nullable: bool, is_array: bool) -> String {
    let base_type = match field_type {
        FieldType::Int => "i32".to_string(),
        FieldType::String => "String".to_string(),
        FieldType::Boolean => "bool".to_string(),
        FieldType::Float => "f64".to_string(),
        FieldType::DateTime => "chrono::NaiveDateTime".to_string(),
        FieldType::Decimal => "rust_decimal::Decimal".to_string(),
        FieldType::Json => "serde_json::Value".to_string(),
        FieldType::Model(name) => format!("{name}Model"),
    };

    let base_type = if is_array {
        format!("Vec<{base_type}>")
    } else {
        base_type
    };

    if nullable {
        format!("Option<{base_type}>")
    } else {
        base_type
    }
}

fn table_name(model: &Model) -> String {
    model
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            crate::ast::ModelAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| pluralize(&model.name))
}

fn column_name(field: &crate::ast::Field) -> String {
    field
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            crate::ast::FieldAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| field.name.clone())
}

fn pluralize(name: &str) -> String {
    format!("{}s", to_snake_case(name))
}

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();

    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            result.push('_');
        }

        result.push(character.to_ascii_lowercase());
    }

    result
}

pub fn write_entities<P: AsRef<Path>>(
    schema: &Schema,
    output_path: P,
) -> std::io::Result<()> {
    let generated = generate(schema);

    if let Some(parent) = output_path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(output_path, generated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        Field, FieldAttribute, FieldType, Model, ModelAttribute, Schema,
    };

    #[test]
    fn generates_basic_entity() {
        let schema = Schema {
            models: vec![Model {
                name: "User".to_string(),
                fields: vec![
                    Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    },
                    Field {
                        name: "name".to_string(),
                        field_type: FieldType::String,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub struct UserModel"));
        assert!(generated.contains("pub id: i32"));
        assert!(generated.contains("pub name: String"));
        assert!(generated.contains("pub struct User;"));
        assert!(generated.contains("impl Entity for User"));
        assert!(generated.contains("const TABLE: &'static str = \"users\""));
        assert!(generated.contains("Field<Self, i32>"));
        assert!(generated.contains("Field<Self, String>"));
    }

    #[test]
    fn uses_map_attribute_for_table_and_column() {
        let schema = Schema {
            models: vec![Model {
                name: "User".to_string(),
                fields: vec![Field {
                    name: "firstName".to_string(),
                    field_type: FieldType::String,
                    nullable: false,
                    is_array: false,
                    attributes: vec![FieldAttribute::Map("first_name".to_string())],
                }],
                attributes: vec![ModelAttribute::Map("users_table".to_string())],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("users_table"));
        assert!(generated.contains("Column::new(\"first_name\")"));
        assert!(generated.contains("Field::new(\"first_name\")"));
    }
}