use crate::ast::{FieldAttribute, FieldType, Model, Schema};
use std::fs;
use std::path::Path;
pub fn generate(schema: &Schema) -> String {
    let mut output = String::new();

    output.push_str(
        "use rustorm::{\n\
        entity::{Column, Entity, RelationKey, RelationLoader, SingleRelationLoader},\n\
        field::Field,\n\
        query::relation::{ManyToMany, ManyToOne, OneToOne, Relation},\n\
        };\n\
        use std::sync::Arc;\n\n",
    );

    let relations = resolve_relations(schema);

    for model in &schema.models {
        generate_model(model, &relations, &mut output);
    }

    output
}

fn generate_model(model: &Model, relations: &[ResolvedRelation], output: &mut String) {
    let model_struct = format!("{}Model", model.name);

    output.push_str("#[derive(Debug, sqlx::FromRow)]\n");
    output.push_str(&format!("pub struct {model_struct} {{\n"));

    for field in &model.fields {
        if matches!(field.field_type, FieldType::Model(_)) {
            let relation = relations.iter().find(|relation| {
                relation.source_model == model.name && relation.field_name == field.name
            });

            let relation_type = relation_model_type(field, relation);

            output.push_str("    #[sqlx(skip)]");
            output.push_str(&format!("    pub {}: {},\n", field.name, relation_type));

            continue;
        }

        let rust_type = rust_type(&field.field_type, field.nullable, field.is_array);

        output.push_str(&format!("    pub {}: {},\n", field.name, rust_type));
    }

    output.push_str("}\n\n");

    output.push_str(&format!("pub struct {};\n\n", model.name));
    generate_create_struct(model, output);

    output.push_str(&format!("impl Entity for {} {{\n", model.name));
    output.push_str(&format!("    type Model = {model_struct};\n"));
    output.push_str(&format!(
        "    const TABLE: &'static str = \"{}\";\n",
        table_name(model)
    ));

    output.push_str("    const COLUMNS: &'static [Column] = &[\n");

    for field in &model.fields {
        if matches!(field.field_type, FieldType::Model(_)) {
            continue;
        }

        output.push_str(&format!(
            "        Column::new(\"{}\"),\n",
            column_name(field)
        ));
    }

    output.push_str("    ];\n");
    output.push_str("}\n\n");

    output.push_str(&format!("impl {} {{\n", model.name));

    for field in &model.fields {
        if matches!(field.field_type, FieldType::Model(_)) {
            continue;
        }

        let rust_type = rust_type(&field.field_type, field.nullable, field.is_array);

        output.push_str("    #[allow(non_upper_case_globals)]\n");
        output.push_str(&format!(
            "    pub const {}: Field<Self, {}> = Field::new(\"{}\");\n\n",
            field.name,
            rust_type,
            column_name(field)
        ));
    }

    for relation in relations {
        if relation.source_model != model.name {
            continue;
        }

        output.push_str("    #[allow(non_upper_case_globals)]\n");

        match relation.kind {
            RelationKind::OneToMany => {
                output.push_str(&format!(
                    "    pub const {}: Relation<Self, {}> = \
                 Relation::new(Self::{}, {}::{});\n\n",
                    relation.field_name,
                    relation.target_model,
                    relation.source_field,
                    relation.target_model,
                    relation.target_field,
                ));
            }

            RelationKind::ManyToOne => {
                output.push_str(&format!(
                    "    pub const {}: Relation<Self, {}, ManyToOne> = \
                 Relation::new(Self::{}, {}::{});\n\n",
                    relation.field_name,
                    relation.target_model,
                    relation.source_field,
                    relation.target_model,
                    relation.target_field,
                ));
            }

            RelationKind::OneToOne => {
                output.push_str(&format!(
                    "    pub const {}: Relation<Self, {}, OneToOne> = \
         Relation::new(Self::{}, {}::{});\n\n",
                    relation.field_name,
                    relation.target_model,
                    relation.source_field,
                    relation.target_model,
                    relation.target_field,
                ));
            }
            RelationKind::ManyToMany => {
                output.push_str(&format!(
                    "    pub const {}: Relation<Self, {}, ManyToMany> =\n",
                    relation.field_name, relation.target_model,
                ));

                output.push_str(&format!(
                    "        Relation::<Self, {}, ManyToMany>::many_to_many(\n",
                    relation.target_model,
                ));

                output.push_str(&format!("            Self::{},\n", relation.source_field,));

                output.push_str(&format!(
                    "            {}::{},\n",
                    relation.target_model, relation.target_field,
                ));

                output.push_str(&format!(
                    "            \"{}\",\n",
                    relation.pivot_table.as_deref().unwrap_or_default(),
                ));

                output.push_str(&format!(
                    "            Column::new(\"{}\"),\n",
                    relation.pivot_from.as_deref().unwrap_or_default(),
                ));

                output.push_str(&format!(
                    "            Column::new(\"{}\"),\n",
                    relation.pivot_to.as_deref().unwrap_or_default(),
                ));

                output.push_str("        );\n\n");
            }
        }
    }

    output.push_str("}\n\n");

    generate_relation_key(model, output);
    generate_relations_for_model(model, relations, output);
}

fn generate_create_struct(model: &Model, output: &mut String) {
    output.push_str(&format!("pub struct {}Create {{\n", model.name));

    for field in &model.fields {
        if matches!(field.field_type, FieldType::Model(_)) {
            continue;
        }

        let is_id = field
            .attributes
            .iter()
            .any(|attribute| matches!(attribute, FieldAttribute::Id));

        if is_id {
            continue;
        }

        let rust_type = rust_type(&field.field_type, field.nullable, field.is_array);

        output.push_str(&format!("    pub {}: {},\n", field.name, rust_type));
    }

    output.push_str("}\n\n");
}

fn generate_relation_key(model: &Model, output: &mut String) {
    output.push_str(&format!("impl RelationKey for {}Model {{\n", model.name));

    output.push_str("    fn relation_key(&self, column: Column) -> Option<i64> {\n");

    output.push_str("        match column.name() {\n");

    for field in &model.fields {
        if !matches!(field.field_type, FieldType::Int) || field.is_array {
            continue;
        }

        let column = column_name(field);

        if field.nullable {
            output.push_str(&format!(
                "            \"{}\" => self.{}.map(|value| value as i64),\n",
                column, field.name
            ));
        } else {
            output.push_str(&format!(
                "            \"{}\" => Some(self.{} as i64),\n",
                column, field.name
            ));
        }
    }

    output.push_str("            _ => None,\n");
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
}

fn generate_relations_for_model(
    model: &Model,
    relations: &[ResolvedRelation],
    output: &mut String,
) {
    for relation in relations {
        if relation.source_model != model.name {
            continue;
        }

        match relation.kind {
            RelationKind::OneToMany => {
                output.push_str(&format!(
                    "impl RelationLoader<{}Model> for {}Model {{\n",
                    relation.target_model, model.name
                ));

                output.push_str(&format!(
                    "    fn load_relation(&mut self, related: Vec<{}Model>) {{\n",
                    relation.target_model
                ));

                output.push_str(&format!(
                    "        self.{} = related;\n",
                    relation.field_name
                ));

                output.push_str("    }\n");
                output.push_str("}\n\n");
            }

            RelationKind::ManyToOne | RelationKind::OneToOne => {
                output.push_str(&format!(
                    "impl SingleRelationLoader<Arc<{}Model>> for {}Model {{\n",
                    relation.target_model, model.name
                ));

                output.push_str(&format!(
                    "    fn load_relation(&mut self, related: Option<Arc<{}Model>>) {{\n",
                    relation.target_model
                ));

                output.push_str(&format!(
                    "        self.{} = related;\n",
                    relation.field_name
                ));

                output.push_str("    }\n");
                output.push_str("}\n\n");
            }
            RelationKind::ManyToMany => {
                output.push_str(&format!(
                    "impl RelationLoader<Arc<{}Model>> for {}Model {{\n",
                    relation.target_model, model.name
                ));

                output.push_str(&format!(
                    "    fn load_relation(&mut self, related: Vec<Arc<{}Model>>) {{\n",
                    relation.target_model
                ));

                output.push_str(&format!(
                    "        self.{} = related;\n",
                    relation.field_name
                ));

                output.push_str("    }\n");
                output.push_str("}\n\n");
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RelationKind {
    OneToMany,
    ManyToOne,
    OneToOne,
    ManyToMany,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedRelation {
    source_model: String,
    field_name: String,
    target_model: String,
    source_field: String,
    target_field: String,
    kind: RelationKind,
    pivot_table: Option<String>,
    pivot_from: Option<String>,
    pivot_to: Option<String>,
}

fn resolve_relations(schema: &Schema) -> Vec<ResolvedRelation> {
    let mut relations = Vec::new();

    for model in &schema.models {
        for field in &model.fields {
            let FieldType::Model(target_model) = &field.field_type else {
                continue;
            };

            let Some(relation) = field
                .attributes
                .iter()
                .find_map(|attribute| match attribute {
                    FieldAttribute::Relation(relation) => Some(relation),
                    _ => None,
                })
            else {
                continue;
            };

            let (kind, source_field, target_field) = if relation.through.is_some() {
                let source_field = model
                    .fields
                    .iter()
                    .find(|field| {
                        field
                            .attributes
                            .iter()
                            .any(|attribute| matches!(attribute, FieldAttribute::Id))
                    })
                    .map(|field| field.name.clone());

                let target = schema
                    .models
                    .iter()
                    .find(|model| model.name == *target_model);

                let target_field = target.and_then(|target_model| {
                    target_model
                        .fields
                        .iter()
                        .find(|field| {
                            field
                                .attributes
                                .iter()
                                .any(|attribute| matches!(attribute, FieldAttribute::Id))
                        })
                        .map(|field| field.name.clone())
                });

                let (Some(source_field), Some(target_field)) = (source_field, target_field) else {
                    continue;
                };

                (RelationKind::ManyToMany, source_field, target_field)
            } else {
                if relation.fields.len() != 1 || relation.references.len() != 1 {
                    continue;
                }

                (
                    RelationKind::ManyToOne,
                    relation.fields[0].clone(),
                    relation.references[0].clone(),
                )
            };

            relations.push(ResolvedRelation {
                source_model: model.name.clone(),
                field_name: field.name.clone(),
                target_model: target_model.clone(),
                source_field,
                target_field,
                kind,
                pivot_table: relation.through.clone(),
                pivot_from: relation.pivot_from.clone(),
                pivot_to: relation.pivot_to.clone(),
            });
        }
    }

    let mut reverse_relations = Vec::new();

    for relation in &relations {
        let Some(target_model) = schema
            .models
            .iter()
            .find(|model| model.name == relation.target_model)
        else {
            continue;
        };

        for field in &target_model.fields {
            let FieldType::Model(referenced_model) = &field.field_type else {
                continue;
            };

            if referenced_model != &relation.source_model {
                continue;
            }

            let kind = if field.is_array {
                RelationKind::OneToMany
            } else {
                RelationKind::OneToOne
            };

            reverse_relations.push(ResolvedRelation {
                source_model: relation.target_model.clone(),
                field_name: field.name.clone(),
                target_model: relation.source_model.clone(),
                source_field: relation.target_field.clone(),
                target_field: relation.source_field.clone(),
                kind,
                pivot_table: relation.pivot_table.clone(),
                pivot_from: relation.pivot_to.clone(),
                pivot_to: relation.pivot_from.clone(),
            });
        }
    }

    relations.extend(reverse_relations);

    relations
}

fn relation_model_type(field: &crate::ast::Field, relation: Option<&ResolvedRelation>) -> String {
    let FieldType::Model(model) = &field.field_type else {
        unreachable!("relation_model_type called for non-model field");
    };
    if let Some(relation) = relation
        && relation.kind == RelationKind::ManyToMany
    {
        return format!("Vec<Arc<{model}Model>>");
    }

    if field.is_array {
        format!("Vec<{model}Model>")
    } else {
        format!("Option<Arc<{model}Model>>")
    }
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

pub fn write_entities<P: AsRef<Path>>(schema: &Schema, output_path: P) -> std::io::Result<()> {
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
        Field, FieldAttribute, FieldType, Model, ModelAttribute, RelationAttribute, Schema,
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
    fn generates_scalar_rust_types() {
        let schema = Schema {
            models: vec![Model {
                name: "Example".to_string(),
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
                    Field {
                        name: "active".to_string(),
                        field_type: FieldType::Boolean,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "score".to_string(),
                        field_type: FieldType::Float,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "createdAt".to_string(),
                        field_type: FieldType::DateTime,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "price".to_string(),
                        field_type: FieldType::Decimal,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "metadata".to_string(),
                        field_type: FieldType::Json,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub id: i32"));
        assert!(generated.contains("pub name: String"));
        assert!(generated.contains("pub active: bool"));
        assert!(generated.contains("pub score: f64"));
        assert!(generated.contains("pub createdAt: chrono::NaiveDateTime"));
        assert!(generated.contains("pub price: rust_decimal::Decimal"));
        assert!(generated.contains("pub metadata: serde_json::Value"));
    }

    #[test]
    fn generates_nullable_rust_types() {
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
                        nullable: true,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "age".to_string(),
                        field_type: FieldType::Int,
                        nullable: true,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "active".to_string(),
                        field_type: FieldType::Boolean,
                        nullable: true,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "score".to_string(),
                        field_type: FieldType::Float,
                        nullable: true,
                        is_array: false,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub name: Option<String>"));
        assert!(generated.contains("pub age: Option<i32>"));
        assert!(generated.contains("pub active: Option<bool>"));
        assert!(generated.contains("pub score: Option<f64>"));
    }

    #[test]
    fn generates_array_rust_types() {
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
                        name: "names".to_string(),
                        field_type: FieldType::String,
                        nullable: false,
                        is_array: true,
                        attributes: vec![],
                    },
                    Field {
                        name: "ages".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: true,
                        attributes: vec![],
                    },
                    Field {
                        name: "active".to_string(),
                        field_type: FieldType::Boolean,
                        nullable: false,
                        is_array: true,
                        attributes: vec![],
                    },
                    Field {
                        name: "scores".to_string(),
                        field_type: FieldType::Float,
                        nullable: false,
                        is_array: true,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub names: Vec<String>"));
        assert!(generated.contains("pub ages: Vec<i32>"));
        assert!(generated.contains("pub active: Vec<bool>"));
        assert!(generated.contains("pub scores: Vec<f64>"));
    }

    #[test]
    fn generates_nullable_array_rust_types() {
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
                        name: "names".to_string(),
                        field_type: FieldType::String,
                        nullable: true,
                        is_array: true,
                        attributes: vec![],
                    },
                    Field {
                        name: "ages".to_string(),
                        field_type: FieldType::Int,
                        nullable: true,
                        is_array: true,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub names: Option<Vec<String>>"));
        assert!(generated.contains("pub ages: Option<Vec<i32>>"));
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

    #[test]
    fn resolves_one_to_many_and_many_to_one_relations() {
        let schema = Schema {
            models: vec![
                Model {
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
                            name: "posts".to_string(),
                            field_type: FieldType::Model("Post".to_string()),
                            nullable: false,
                            is_array: true,
                            attributes: vec![],
                        },
                    ],
                    attributes: vec![],
                },
                Model {
                    name: "Post".to_string(),
                    fields: vec![
                        Field {
                            name: "id".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Id],
                        },
                        Field {
                            name: "userId".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![],
                        },
                        Field {
                            name: "user".to_string(),
                            field_type: FieldType::Model("User".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    name: None,
                                    fields: vec!["userId".to_string()],
                                    references: vec!["id".to_string()],
                                    through: None,
                                    pivot_from: None,
                                    pivot_to: None,
                                    on_delete: None,
                                },
                            )],
                        },
                    ],
                    attributes: vec![],
                },
            ],
        };

        let relations = resolve_relations(&schema);

        assert!(relations.contains(&ResolvedRelation {
            source_model: "Post".to_string(),
            field_name: "user".to_string(),
            target_model: "User".to_string(),
            source_field: "userId".to_string(),
            target_field: "id".to_string(),
            kind: RelationKind::ManyToOne,
            pivot_table: None,
            pivot_from: None,
            pivot_to: None,
        }));

        assert!(relations.contains(&ResolvedRelation {
            source_model: "Post".to_string(),
            field_name: "user".to_string(),
            target_model: "User".to_string(),
            source_field: "userId".to_string(),
            target_field: "id".to_string(),
            kind: RelationKind::ManyToOne,
            pivot_table: None,
            pivot_from: None,
            pivot_to: None,
        }));
    }

    #[test]
    fn generates_one_to_many_and_many_to_one_relations() {
        let schema = Schema {
            models: vec![
                Model {
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
                            name: "posts".to_string(),
                            field_type: FieldType::Model("Post".to_string()),
                            nullable: false,
                            is_array: true,
                            attributes: vec![],
                        },
                    ],
                    attributes: vec![],
                },
                Model {
                    name: "Post".to_string(),
                    fields: vec![
                        Field {
                            name: "id".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Id],
                        },
                        Field {
                            name: "userId".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![],
                        },
                        Field {
                            name: "user".to_string(),
                            field_type: FieldType::Model("User".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    name: None,
                                    fields: vec!["userId".to_string()],
                                    references: vec!["id".to_string()],
                                    through: None,
                                    pivot_from: None,
                                    pivot_to: None,
                                    on_delete: None,
                                },
                            )],
                        },
                    ],
                    attributes: vec![],
                },
            ],
        };

        let generated = generate(&schema);

        // User.posts
        assert!(generated.contains("pub posts: Vec<PostModel>"));

        assert!(generated.contains("impl RelationLoader<PostModel> for UserModel"));

        assert!(generated.contains("self.posts = related;"));

        assert!(generated.contains(
            "pub const posts: Relation<Self, Post> = Relation::new(Self::id, Post::userId);"
        ));

        // Post.user
        assert!(generated.contains("pub user: Option<Arc<UserModel>>"));

        assert!(generated.contains("impl SingleRelationLoader<Arc<UserModel>> for PostModel"));

        assert!(generated.contains("self.user = related;"));

        assert!(generated.contains(
        "pub const user: Relation<Self, User, ManyToOne> = Relation::new(Self::userId, User::id);"
    ));
    }
    #[test]
    fn generates_relation_key() {
        let schema = Schema {
            models: vec![Model {
                name: "Post".to_string(),
                fields: vec![
                    Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    },
                    Field {
                        name: "userId".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Map("user_id".to_string())],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("impl RelationKey for PostModel"));
        assert!(generated.contains("fn relation_key(&self, column: Column) -> Option<i64>"));
        assert!(generated.contains(r#""id" => Some(self.id as i64)"#));
        assert!(generated.contains(r#""user_id" => Some(self.userId as i64)"#));
        assert!(generated.contains("_ => None"));
    }
    #[test]
    fn generates_one_to_one_relation() {
        let schema = Schema {
            models: vec![
                Model {
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
                            name: "profile".to_string(),
                            field_type: FieldType::Model("Profile".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![],
                        },
                    ],
                    attributes: vec![],
                },
                Model {
                    name: "Profile".to_string(),
                    fields: vec![
                        Field {
                            name: "id".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Id],
                        },
                        Field {
                            name: "userId".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![],
                        },
                        Field {
                            name: "user".to_string(),
                            field_type: FieldType::Model("User".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    name: None,
                                    fields: vec!["userId".to_string()],
                                    references: vec!["id".to_string()],
                                    through: None,
                                    pivot_from: None,
                                    pivot_to: None,
                                    on_delete: None,
                                },
                            )],
                        },
                    ],
                    attributes: vec![],
                },
            ],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub profile: Option<Arc<ProfileModel>>"));
        assert!(generated.contains(
            "pub const profile: Relation<Self, Profile, OneToOne> = \
Relation::new(Self::id, Profile::userId);"
        ));
        assert!(generated.contains("impl SingleRelationLoader<Arc<ProfileModel>> for UserModel"));
        assert!(generated.contains("self.profile = related;"));
    }
    #[test]
    fn resolves_many_to_many_relation() {
        let schema = Schema {
            models: vec![
                Model {
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
                            name: "roles".to_string(),
                            field_type: FieldType::Model("Role".to_string()),
                            nullable: false,
                            is_array: true,
                            attributes: vec![FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    name: Some("UserRoles".to_string()),
                                    fields: vec![],
                                    references: vec![],
                                    through: Some("user_roles".to_string()),
                                    pivot_from: Some("user_id".to_string()),
                                    pivot_to: Some("role_id".to_string()),
                                    on_delete: None,
                                },
                            )],
                        },
                    ],
                    attributes: vec![],
                },
                Model {
                    name: "Role".to_string(),
                    fields: vec![Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    }],
                    attributes: vec![],
                },
            ],
        };

        let relations = resolve_relations(&schema);
        dbg!(&relations);

        assert!(relations.contains(&ResolvedRelation {
            source_model: "User".to_string(),
            field_name: "roles".to_string(),
            target_model: "Role".to_string(),
            source_field: "id".to_string(),
            target_field: "id".to_string(),
            kind: RelationKind::ManyToMany,
            pivot_table: Some("user_roles".to_string()),
            pivot_from: Some("user_id".to_string()),
            pivot_to: Some("role_id".to_string()),
        }));
    }
    #[test]
    fn generates_many_to_many_relation() {
        let schema = Schema {
            models: vec![
                Model {
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
                            name: "roles".to_string(),
                            field_type: FieldType::Model("Role".to_string()),
                            nullable: false,
                            is_array: true,
                            attributes: vec![FieldAttribute::Relation(RelationAttribute {
                                name: Some("UserRoles".to_string()),
                                fields: vec![],
                                references: vec![],
                                through: Some("user_roles".to_string()),
                                pivot_from: Some("user_id".to_string()),
                                pivot_to: Some("role_id".to_string()),
                                on_delete: None,
                            })],
                        },
                    ],
                    attributes: vec![],
                },
                Model {
                    name: "Role".to_string(),
                    fields: vec![Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    }],
                    attributes: vec![],
                },
            ],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub roles: Vec<Arc<RoleModel>>"));

        assert!(generated.contains("impl RelationLoader<Arc<RoleModel>> for UserModel"));

        assert!(generated.contains("self.roles = related;"));

        assert!(generated.contains("pub const roles: Relation<Self, Role, ManyToMany> ="));

        assert!(generated.contains("Relation::<Self, Role, ManyToMany>::many_to_many("));

        assert!(generated.contains("Self::id"));
        assert!(generated.contains("Role::id"));
        assert!(generated.contains("\"user_roles\""));
        assert!(generated.contains("Column::new(\"user_id\")"));
        assert!(generated.contains("Column::new(\"role_id\")"));
    }
    #[test]
    fn generates_create_struct() {
        let schema = Schema {
            models: vec![Model {
                name: "User".to_string(),
                fields: vec![
                    Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![
                            FieldAttribute::Id,
                            FieldAttribute::Default(crate::ast::DefaultValue::AutoIncrement),
                        ],
                    },
                    Field {
                        name: "name".to_string(),
                        field_type: FieldType::String,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                    Field {
                        name: "email".to_string(),
                        field_type: FieldType::String,
                        nullable: true,
                        is_array: false,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema);

        assert!(generated.contains("pub struct UserCreate {"));

        let create_start = generated
            .find("pub struct UserCreate")
            .expect("UserCreate should be generated");

        let create_end = generated[create_start..]
            .find("\n}\n")
            .map(|offset| create_start + offset + 3)
            .expect("UserCreate should close");

        let create_section = &generated[create_start..create_end];

        assert!(create_section.contains("pub name: String,"));
        assert!(create_section.contains("pub email: Option<String>,"));
        assert!(!create_section.contains("pub id: i32"));
    }
}
