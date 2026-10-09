use crate::ast::{
    DefaultValue, Field, FieldAttribute, FieldType, Model, ModelAttribute, ReferentialAction,
    Schema,
};
use std::fmt::Write;

pub fn generate_migration(schema: &Schema) -> Result<String, String> {
    let mut sql = String::new();

    for model in &schema.models {
        validate_model(model)?;

        let table = table_name(model);
        let mut definitions = Vec::new();
        let mut primary_keys = Vec::new();

        for field in &model.fields {
            if matches!(field.field_type, FieldType::Model(_)) {
                continue;
            }
            let column = column_name(field);
            let ty = sql_type(field)?;

            let is_id = field
                .attributes
                .iter()
                .any(|a| matches!(a, FieldAttribute::Id));

            if is_id {
                primary_keys.push(column.clone());
            }

            let mut definition = format!("{} {ty}", quote_identifier(&column));

            if !field.nullable {
                definition.push_str(" NOT NULL");
            }

            if let Some(default) = field
                .attributes
                .iter()
                .find_map(|attribute| match attribute {
                    FieldAttribute::Default(value) => Some(value),
                    _ => None,
                })
                && let Some(expression) = default_sql(default, field)?
            {
                write!(definition, " DEFAULT {expression}").unwrap();
            }

            if field
                .attributes
                .iter()
                .any(|a| matches!(a, FieldAttribute::Unique))
            {
                write!(definition, " UNIQUE").unwrap();
            }

            definitions.push(definition);
        }

        let columns = primary_keys
            .iter()
            .map(|name| quote_identifier(name))
            .collect::<Vec<_>>()
            .join(", ");

        definitions.push(format!("PRIMARY KEY ({columns})"));

        for field in &model.fields {
            let Some(relation) = field.attributes.iter().find_map(|attribute| {
                if let FieldAttribute::Relation(relation) = attribute {
                    Some(relation)
                } else {
                    None
                }
            }) else {
                continue;
            };

            if relation.fields.is_empty() {
                continue;
            }

            let FieldType::Model(target_name) = &field.field_type else {
                return Err(format!(
                    "Relation '{}.{}' must reference a model",
                    model.name, field.name
                ));
            };

            let target = schema
                .models
                .iter()
                .find(|candidate| candidate.name == *target_name)
                .ok_or_else(|| {
                    format!(
                        "Unknown target model '{}' in relation '{}.{}'",
                        target_name, model.name, field.name
                    )
                })?;

            if relation.fields.len() != relation.references.len() {
                return Err(format!(
                    "Relation '{}.{}' has mismatched fields and references",
                    model.name, field.name
                ));
            }

            let mut source_columns = Vec::new();
            let mut target_columns = Vec::new();

            for (source_field, target_field) in relation.fields.iter().zip(&relation.references) {
                let source = model
                    .fields
                    .iter()
                    .find(|f| f.name == *source_field)
                    .ok_or_else(|| {
                        format!(
                            "Unknown foreign-key field '{}.{}'",
                            model.name, source_field
                        )
                    })?;

                let target_column = target
                    .fields
                    .iter()
                    .find(|f| f.name == *target_field)
                    .ok_or_else(|| {
                        format!(
                            "Unknown referenced field '{}.{}'",
                            target.name, target_field
                        )
                    })?;

                source_columns.push(quote_identifier(&column_name(source)));
                target_columns.push(quote_identifier(&column_name(target_column)));
            }

            let action = relation
                .on_delete
                .as_ref()
                .map(referential_action)
                .unwrap_or("");

            let constraint = format!(
                "FOREIGN KEY ({}) REFERENCES {} ({}){}",
                source_columns.join(", "),
                quote_identifier(&table_name(target)),
                target_columns.join(", "),
                action
            );

            definitions.push(constraint);
        }

        writeln!(
            sql,
            "CREATE TABLE {} (\n    {}\n);\n",
            quote_identifier(&table),
            definitions
                .iter()
                .map(|definition| format!("    {definition}"))
                .collect::<Vec<_>>()
                .join(",\n")
        )
        .unwrap();
    }

    for model in &schema.models {
        for attribute in &model.attributes {
            match attribute {
                ModelAttribute::Index(fields) => {
                    sql.push_str(&index_sql(model, fields, false)?);
                }
                ModelAttribute::Unique(fields) => {
                    sql.push_str(&index_sql(model, fields, true)?);
                }
                ModelAttribute::Map(_) => {}
            }
        }
    }

    Ok(sql)
}

fn validate_model(model: &Model) -> Result<(), String> {
    let ids = model
        .fields
        .iter()
        .filter(|field| {
            field
                .attributes
                .iter()
                .any(|attribute| matches!(attribute, FieldAttribute::Id))
        })
        .count();

    if ids == 0 {
        return Err(format!("Model '{}' has no primary key", model.name));
    }

    if ids > 1 {
        return Err(format!(
            "Model '{}' has multiple primary keys; composite primary keys are not supported yet",
            model.name
        ));
    }

    Ok(())
}

fn sql_type(field: &Field) -> Result<String, String> {
    let ty = match &field.field_type {
        FieldType::Int => "INTEGER",
        FieldType::String => "TEXT",
        FieldType::Boolean => "BOOLEAN",
        FieldType::Float => "DOUBLE PRECISION",
        FieldType::DateTime => "TIMESTAMP",
        FieldType::Decimal => "NUMERIC",
        FieldType::Json => "JSONB",
        FieldType::Model(_) => {
            return Err(format!(
                "Model relation '{}' is not a scalar column",
                field.name
            ));
        }
    };

    if field.is_array {
        return Ok(format!("{ty}[]"));
    }

    let auto_increment = field.attributes.iter().any(|attribute| {
        matches!(
            attribute,
            FieldAttribute::Default(DefaultValue::AutoIncrement)
        )
    });

    if auto_increment {
        if matches!(field.field_type, FieldType::Int) {
            return Ok("SERIAL".to_string());
        }

        return Err(format!(
            "Auto-increment is only supported for Int fields ('{}')",
            field.name
        ));
    }

    Ok(ty.to_string())
}

fn default_sql(default: &DefaultValue, field: &Field) -> Result<Option<String>, String> {
    let value = match default {
        DefaultValue::AutoIncrement => return Ok(None),
        DefaultValue::Now => "CURRENT_TIMESTAMP".to_string(),
        DefaultValue::Boolean(value) => value.to_string(),
        DefaultValue::Integer(value) => value.to_string(),
        DefaultValue::Float(value) if value.is_finite() => value.to_string(),
        DefaultValue::Float(_) => {
            return Err(format!(
                "Non-finite default value for field '{}'",
                field.name
            ));
        }
        DefaultValue::String(value) => {
            format!("'{}'", value.replace('\'', "''"))
        }
        // Cuid and application-managed updated timestamps are generated
        // by application code, not by PostgreSQL.
        DefaultValue::Cuid | DefaultValue::Identifier(_) => return Ok(None),
    };

    Ok(Some(value))
}

fn index_sql(model: &Model, fields: &[String], unique: bool) -> Result<String, String> {
    if fields.is_empty() {
        return Err(format!("Empty index declaration on model '{}'", model.name));
    }

    let mut columns = Vec::new();

    for name in fields {
        let field = model
            .fields
            .iter()
            .find(|field| field.name == *name)
            .ok_or_else(|| format!("Unknown index field '{}.{}'", model.name, name))?;

        columns.push(quote_identifier(&column_name(field)));
    }

    let kind = if unique { "UNIQUE " } else { "" };
    let index_name = format!(
        "{}_{}_idx",
        table_name(model),
        fields
            .iter()
            .map(|name| snake_case(name))
            .collect::<Vec<_>>()
            .join("_")
    );

    let quoted_index = quote_identifier(&index_name);
    let quoted_table = quote_identifier(&table_name(model));

    Ok(format!(
        "CREATE {kind}INDEX {quoted_index} ON {quoted_table} ({});\n",
        columns.join(", ")
    ))
}

fn referential_action(action: &ReferentialAction) -> &'static str {
    match action {
        ReferentialAction::Cascade => " ON DELETE CASCADE",
        ReferentialAction::Restrict => " ON DELETE RESTRICT",
        ReferentialAction::SetNull => " ON DELETE SET NULL",
        ReferentialAction::NoAction => " ON DELETE NO ACTION",
    }
}

fn table_name(model: &Model) -> String {
    model
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            ModelAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| snake_case(&model.name))
}

fn column_name(field: &Field) -> String {
    field
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            FieldAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| snake_case(&field.name))
}
fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn snake_case(name: &str) -> String {
    let mut result = String::new();

    for (index, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                result.push('_');
            }
            result.extend(ch.to_lowercase());
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(
        name: &str,
        field_type: FieldType,
        nullable: bool,
        attributes: Vec<FieldAttribute>,
    ) -> Field {
        Field {
            name: name.to_string(),
            field_type,
            nullable,
            is_array: false,
            attributes,
        }
    }

    #[test]
    fn generates_table_columns_and_primary_key() {
        let schema = Schema {
            models: vec![Model {
                name: "User".to_string(),
                fields: vec![
                    field("id", FieldType::Int, false, vec![FieldAttribute::Id]),
                    field("email", FieldType::String, false, vec![]),
                    field("nickname", FieldType::String, true, vec![]),
                ],
                attributes: vec![],
            }],
        };

        let sql = generate_migration(&schema).unwrap();

        assert!(sql.contains("CREATE TABLE \"user\""));
        assert!(sql.contains("\"id\" INTEGER NOT NULL"));
        assert!(sql.contains("\"email\" TEXT NOT NULL"));
        assert!(sql.contains("\"nickname\" TEXT"));
        assert!(sql.contains("PRIMARY KEY (\"id\")"));
    }

    #[test]
    fn generates_mapped_names_defaults_and_indexes() {
        let schema = Schema {
            models: vec![Model {
                name: "UserProfile".to_string(),
                fields: vec![
                    field(
                        "id",
                        FieldType::Int,
                        false,
                        vec![
                            FieldAttribute::Id,
                            FieldAttribute::Default(DefaultValue::AutoIncrement),
                        ],
                    ),
                    field(
                        "displayName",
                        FieldType::String,
                        false,
                        vec![
                            FieldAttribute::Map("display_name".to_string()),
                            FieldAttribute::Default(DefaultValue::String("guest".to_string())),
                        ],
                    ),
                ],
                attributes: vec![
                    ModelAttribute::Map("user_profiles".to_string()),
                    ModelAttribute::Index(vec!["displayName".to_string()]),
                ],
            }],
        };

        let sql = generate_migration(&schema).unwrap();

        assert!(sql.contains("CREATE TABLE \"user_profiles\""));
        assert!(sql.contains("\"display_name\" TEXT NOT NULL DEFAULT 'guest'"));
        assert!(sql.contains("\"id\" SERIAL NOT NULL"));
        assert!(sql.contains("CREATE INDEX \"user_profiles_display_name_idx\""));
    }

    #[test]
    fn escapes_quotes_in_string_defaults() {
        let schema = Schema {
            models: vec![Model {
                name: "User".to_string(),
                fields: vec![
                    field("id", FieldType::Int, false, vec![FieldAttribute::Id]),
                    field(
                        "name",
                        FieldType::String,
                        false,
                        vec![FieldAttribute::Default(DefaultValue::String(
                            "Fak'hir".to_string(),
                        ))],
                    ),
                ],
                attributes: vec![],
            }],
        };

        let sql = generate_migration(&schema).unwrap();

        assert!(sql.contains("DEFAULT 'Fak''hir'"));
    }
    #[test]
    fn generates_single_foreign_key() {
        let schema = Schema {
            models: vec![
                Model {
                    name: "User".to_string(),
                    fields: vec![
                        field("id", FieldType::Int, false, vec![FieldAttribute::Id]),
                    ],
                    attributes: vec![],
                },
                Model {
                    name: "Post".to_string(),
                    fields: vec![
                        field("id", FieldType::Int, false, vec![FieldAttribute::Id]),
                        field(
                            "userId",
                            FieldType::Int,
                            false,
                            vec![FieldAttribute::Map("user_id".to_string())],
                        ),
                        field(
                            "user",
                            FieldType::Model("User".to_string()),
                            false,
                            vec![FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    fields: vec!["userId".to_string()],
                                    references: vec!["id".to_string()],
                                    on_delete: None,
                                },
                            )],
                        ),
                    ],
                    attributes: vec![],
                },
            ],
        };

        let sql = generate_migration(&schema).unwrap();

        assert!(sql.contains(
            r#"FOREIGN KEY ("user_id") REFERENCES "user" ("id")"#
        ));
    }

    #[test]
    fn generates_composite_foreign_key_as_one_constraint() {
        let schema = Schema {
            models: vec![
                Model {
                    name: "Parent".to_string(),
                    fields: vec![
                        field("tenantId", FieldType::Int, false, vec![FieldAttribute::Id]),
                        field("id", FieldType::Int, false, vec![FieldAttribute::Id]),
                    ],
                    attributes: vec![],
                },
                Model {
                    name: "Child".to_string(),
                    fields: vec![
                        field("id", FieldType::Int, false, vec![FieldAttribute::Id]),
                        field("tenantId", FieldType::Int, false, vec![]),
                        field("parentId", FieldType::Int, false, vec![]),
                        field(
                            "parent",
                            FieldType::Model("Parent".to_string()),
                            false,
                            vec![FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    fields: vec![
                                        "tenantId".to_string(),
                                        "parentId".to_string(),
                                    ],
                                    references: vec![
                                        "tenantId".to_string(),
                                        "id".to_string(),
                                    ],
                                    on_delete: None,
                                },
                            )],
                        ),
                    ],
                    attributes: vec![],
                },
            ],
        };

        let sql = generate_migration(&schema).unwrap();

        assert!(sql.contains(
            r#"FOREIGN KEY ("tenant_id", "parent_id") REFERENCES "parent" ("tenant_id", "id")"#
        ));

        assert_eq!(sql.matches("FOREIGN KEY").count(), 1);
    }


}
