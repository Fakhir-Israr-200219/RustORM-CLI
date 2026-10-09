use crate::ast::{FieldAttribute, FieldType, Model, Schema};
use std::fs;
use std::path::Path;
#[derive(Debug, Clone, PartialEq)]
pub enum GenerateError {
    UnsupportedArrayField { model: String, field: String },
}

pub fn generate(schema: &Schema) -> Result<String, GenerateError> {
    let relations = resolve_relations(schema);

    let has_one_to_many = relations
        .iter()
        .any(|relation| matches!(relation.kind, RelationKind::OneToMany));

    let has_many_to_one = relations
        .iter()
        .any(|relation| matches!(relation.kind, RelationKind::ManyToOne));

    let has_one_to_one = relations
        .iter()
        .any(|relation| matches!(relation.kind, RelationKind::OneToOne));

    let has_many_to_many = relations
        .iter()
        .any(|relation| matches!(relation.kind, RelationKind::ManyToMany));

    let mut output = String::new();

    output.push_str("use rustorm::{\n    entity::{Column, Entity, RelationKey");

    if has_one_to_many || has_many_to_many {
        output.push_str(", RelationLoader");
    }

    if has_many_to_one || has_one_to_one {
        output.push_str(", SingleRelationLoader");
    }

    output.push_str("},\n    field::Field,\n");

    if has_one_to_many || has_many_to_one || has_one_to_one || has_many_to_many {
        output.push_str("    query::relation::{Relation");

        if has_many_to_many {
            output.push_str(", ManyToMany");
        }

        if has_many_to_one {
            output.push_str(", ManyToOne");
        }

        if has_one_to_one {
            output.push_str(", OneToOne");
        }

        output.push_str("},\n");
    }

    output.push_str("};\n");

    if has_many_to_one || has_one_to_one || has_many_to_many {
        output.push_str("use std::sync::Arc;\n");
    }

    output.push('\n');

    for model in &schema.models {
        generate_model(model, &relations, &mut output)?;
    }

    Ok(output)
}

fn validate_model_crud_support(model: &Model) -> Result<(), GenerateError> {
    for field in &model.fields {
        if field.is_array && !matches!(field.field_type, FieldType::Model(_)) {
            return Err(GenerateError::UnsupportedArrayField {
                model: model.name.clone(),
                field: field.name.clone(),
            });
        }
    }

    Ok(())
}
fn generate_update_data(model: &Model, output: &mut String) {
    output.push_str(&format!(
        "impl rustorm::executor::UpdateData<{}> for {}Update {{\n",
        model.name, model.name
    ));

    output.push_str("    fn columns(&self) -> Vec<&'static str> {\n");
    output.push_str("        let mut columns = Vec::new();\n");

    for field in update_data_fields(model) {
        output.push_str(&format!(
            "        if self.{}.is_some() {{\n",
            field_identifier(field)
        ));
        output.push_str(&format!(
            "            columns.push(\"{}\");\n",
            column_name(field)
        ));
        output.push_str("        }\n");
    }

    output.push_str("        columns\n");
    output.push_str("    }\n\n");

    output.push_str("    fn values(&self) -> Vec<rustorm::value::BindValue> {\n");
    output.push_str("        let mut values = Vec::new();\n");

    for field in update_data_fields(model) {
        let identifier = field_identifier(field);

        output.push_str(&format!(
            "        if let Some(value) = &self.{identifier} {{\n"
        ));

        if field.nullable {
            output.push_str("            match value {\n");
            output.push_str("                Some(value) => values.push(");
            generate_non_null_bind_value(field, "value", output);
            output.push_str("),\n");
            output.push_str(
                "                None => values.push(rustorm::value::BindValue::Null),\n",
            );
            output.push_str("            }\n");
        } else {
            output.push_str("            values.push(");
            generate_non_null_bind_value(field, "value", output);
            output.push_str(");\n");
        }

        output.push_str("        }\n");
    }

    output.push_str("        values\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
}
fn update_data_fields(model: &Model) -> Vec<&crate::ast::Field> {
    model
        .fields
        .iter()
        .filter(|field| {
            !matches!(field.field_type, FieldType::Model(_))
                && !field
                    .attributes
                    .iter()
                    .any(|attribute| matches!(attribute, FieldAttribute::Id))
        })
        .collect()
}
fn generate_update_struct(model: &Model, output: &mut String) {
    output.push_str(&format!("pub struct {}Update {{\n", model.name));

    for field in update_data_fields(model) {
        let rust_type = if field.nullable {
            format!(
                "Option<Option<{}>>",
                rust_type(&field.field_type, false, field.is_array)
            )
        } else {
            format!(
                "Option<{}>",
                rust_type(&field.field_type, false, field.is_array)
            )
        };

        output.push_str(&format!(
            "    pub {}: {},\n",
            field_identifier(field),
            rust_type
        ));
    }

    output.push_str("}\n\n");
}
fn generate_model(
    model: &Model,
    relations: &[ResolvedRelation],
    output: &mut String,
) -> Result<(), GenerateError> {
    let model_struct = format!("{}Model", model.name);

    output.push_str("#[derive(Debug, sqlx::FromRow)]\n");
    output.push_str(&format!("pub struct {model_struct} {{\n"));

    for field in &model.fields {
        if matches!(field.field_type, FieldType::Model(_)) {
            let relation = relations.iter().find(|relation| {
                relation.source_model == model.name && relation.field_name == field.name
            });

            let relation_type = relation_model_type(field, relation);

            output.push_str("    #[sqlx(skip)]\n");
            output.push_str(&format!(
                "    pub {}: {},\n",
                field_identifier(field),
                relation_type
            ));

            continue;
        }

        let rust_type = rust_type(&field.field_type, field.nullable, field.is_array);

        output.push_str(&format!(
            "    pub {}: {},\n",
            field_identifier(field),
            rust_type
        ));
    }

    output.push_str("}\n\n");

    output.push_str(&format!("pub struct {};\n\n", model.name));
    validate_model_crud_support(model)?;

    generate_create_struct(model, output);
    generate_insert_data(model, output);
    generate_update_struct(model, output);
    generate_update_data(model, output);

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
            field_identifier(field),
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
                    rust_identifier(&relation.field_name),
                    relation.target_model,
                    rust_identifier(&relation.source_field),
                    relation.target_model,
                    rust_identifier(&relation.target_field),
                ));
            }

            RelationKind::ManyToOne => {
                output.push_str(&format!(
                    "    pub const {}: Relation<Self, {}, ManyToOne> = \
         Relation::new(Self::{}, {}::{});\n\n",
                    rust_identifier(&relation.field_name),
                    relation.target_model,
                    rust_identifier(&relation.source_field),
                    relation.target_model,
                    rust_identifier(&relation.target_field),
                ));
            }

            RelationKind::OneToOne => {
                output.push_str(&format!(
                    "    pub const {}: Relation<Self, {}, OneToOne> = \
         Relation::new(Self::{}, {}::{});\n\n",
                    rust_identifier(&relation.field_name),
                    relation.target_model,
                    rust_identifier(&relation.source_field),
                    relation.target_model,
                    rust_identifier(&relation.target_field),
                ));
            }
            RelationKind::ManyToMany => {
                output.push_str(&format!(
                    "    pub const {}: Relation<Self, {}, ManyToMany> =\n",
                    rust_identifier(&relation.field_name),
                    relation.target_model,
                ));

                output.push_str(&format!(
                    "        Relation::<Self, {}, ManyToMany>::many_to_many(\n",
                    relation.target_model,
                ));

                output.push_str(&format!(
                    "            Self::{},\n",
                    rust_identifier(&relation.source_field),
                ));

                output.push_str(&format!(
                    "            {}::{},\n",
                    relation.target_model,
                    rust_identifier(&relation.target_field),
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
    Ok(())
}

fn generate_create_struct(model: &Model, output: &mut String) {
    output.push_str(&format!("pub struct {}Create {{\n", model.name));

    for field in &model.fields {
        if matches!(field.field_type, FieldType::Model(_)) || field.is_array {
            continue;
        }
        if field.is_array {
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

        output.push_str(&format!(
            "    pub {}: {},\n",
            field_identifier(field),
            rust_type
        ));
    }

    output.push_str("}\n\n");
}
fn generate_insert_data(model: &Model, output: &mut String) {
    output.push_str(&format!(
        "impl rustorm::executor::InsertData<{}> for {}Create {{\n",
        model.name, model.name
    ));

    output.push_str("    fn columns(&self) -> &'static [&'static str] {\n");
    output.push_str("        &[");

    let fields = create_data_fields(model);

    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }

        output.push_str(&format!("\"{}\"", column_name(field)));
    }

    output.push_str("]\n");
    output.push_str("    }\n\n");

    output.push_str("    fn values(&self) -> Vec<rustorm::value::BindValue> {\n");
    output.push_str("        vec![\n");

    for field in &fields {
        output.push_str("            ");
        generate_bind_value(field, output);
        output.push_str(",\n");
    }

    output.push_str("        ]\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
}

fn create_data_fields(model: &Model) -> Vec<&crate::ast::Field> {
    model
        .fields
        .iter()
        .filter(|field| {
            !matches!(field.field_type, FieldType::Model(_))
                && !field
                    .attributes
                    .iter()
                    .any(|attribute| matches!(attribute, FieldAttribute::Id))
        })
        .collect()
}

fn generate_bind_value(field: &crate::ast::Field, output: &mut String) {
    let value = format!("self.{}", field_identifier(field));

    if field.nullable {
        output.push_str(&format!("match &{value} {{\n"));

        output.push_str("                Some(value) => ");
        generate_non_null_bind_value(field, "value", output);
        output.push_str(",\n");

        output.push_str("                None => rustorm::value::BindValue::Null,\n");

        output.push_str("            }");
    } else {
        generate_non_null_bind_value(field, &format!("&{value}"), output);
    }
}

fn generate_non_null_bind_value(field: &crate::ast::Field, value: &str, output: &mut String) {
    let bind_value = match field.field_type {
        FieldType::Int => {
            format!("rustorm::value::BindValue::I64(i64::from(*({value})))")
        }
        FieldType::String => {
            format!("rustorm::value::BindValue::String(({value}).clone())")
        }
        FieldType::Boolean => {
            format!("rustorm::value::BindValue::Boolean(*({value}))")
        }
        FieldType::Float => {
            format!("rustorm::value::BindValue::F64(*({value}))")
        }
        FieldType::DateTime => {
            format!("rustorm::value::BindValue::DateTime(({value}).clone())")
        }
        FieldType::Decimal => {
            format!("rustorm::value::BindValue::Decimal(({value}).clone())")
        }
        FieldType::Json => {
            format!("rustorm::value::BindValue::Json(({value}).clone())")
        }
        FieldType::Model(_) => unreachable!("model fields are excluded"),
    };

    output.push_str(&bind_value);
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
                column,
                field_identifier(field)
            ));
        } else {
            output.push_str(&format!(
                "            \"{}\" => Some(self.{} as i64),\n",
                column,
                field_identifier(field)
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
                    rust_identifier(&relation.field_name)
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
                    rust_identifier(&relation.field_name)
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
                    rust_identifier(&relation.field_name)
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

fn field_identifier(field: &crate::ast::Field) -> String {
    to_snake_case(&field.name)
}
fn rust_identifier(name: &str) -> String {
    to_snake_case(name)
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

pub fn write_entities<P: AsRef<Path>>(schema: &Schema, output_path: P) -> Result<(), String> {
    let generated = generate(schema).map_err(|error| format!("generation failed: {error:?}"))?;

    if let Some(parent) = output_path.as_ref().parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create output directory: {error}"))?;
    }

    fs::write(output_path, generated)
        .map_err(|error| format!("failed to write generated entities: {error}"))
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

        let generated = generate(&schema).unwrap();

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

        let generated = generate(&schema).unwrap();

        assert!(generated.contains("pub id: i32"));
        assert!(generated.contains("pub name: String"));
        assert!(generated.contains("pub active: bool"));
        assert!(generated.contains("pub score: f64"));
        assert!(generated.contains("pub created_at: chrono::NaiveDateTime"));
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

        let generated = generate(&schema).unwrap();

        assert!(generated.contains("pub name: Option<String>"));
        assert!(generated.contains("pub age: Option<i32>"));
        assert!(generated.contains("pub active: Option<bool>"));
        assert!(generated.contains("pub score: Option<f64>"));
    }

    #[test]
    fn generates_array_rust_types() {
        assert_eq!(rust_type(&FieldType::String, false, true), "Vec<String>");

        assert_eq!(rust_type(&FieldType::Int, false, true), "Vec<i32>");

        assert_eq!(rust_type(&FieldType::Boolean, false, true), "Vec<bool>");

        assert_eq!(rust_type(&FieldType::Float, false, true), "Vec<f64>");
    }
    #[test]
    fn generates_nullable_array_rust_types() {
        assert_eq!(
            rust_type(&FieldType::String, true, true),
            "Option<Vec<String>>"
        );

        assert_eq!(rust_type(&FieldType::Int, true, true), "Option<Vec<i32>>");
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

        let generated = generate(&schema).unwrap();

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
            source_model: "User".to_string(),
            field_name: "posts".to_string(),
            target_model: "Post".to_string(),
            source_field: "id".to_string(),
            target_field: "userId".to_string(),
            kind: RelationKind::OneToMany,
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

        let generated = generate(&schema).unwrap();

        // User.posts
        assert!(generated.contains("pub posts: Vec<PostModel>"));

        assert!(generated.contains("impl RelationLoader<PostModel> for UserModel"));

        assert!(generated.contains("self.posts = related;"));

        assert!(generated.contains(
            "pub const posts: Relation<Self, Post> = Relation::new(Self::id, Post::user_id);"
        ));

        // Post.user
        assert!(generated.contains("pub user: Option<Arc<UserModel>>"));

        assert!(generated.contains("impl SingleRelationLoader<Arc<UserModel>> for PostModel"));

        assert!(generated.contains("self.user = related;"));

        assert!(generated.contains(
    "pub const user: Relation<Self, User, ManyToOne> = Relation::new(Self::user_id, User::id);"
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

        let generated = generate(&schema).unwrap();

        assert!(generated.contains("impl RelationKey for PostModel"));
        assert!(generated.contains("fn relation_key(&self, column: Column) -> Option<i64>"));
        assert!(generated.contains(r#""id" => Some(self.id as i64)"#));
        assert!(generated.contains(r#""user_id" => Some(self.user_id as i64)"#));
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

        let generated = generate(&schema).unwrap();

        assert!(generated.contains("pub profile: Option<Arc<ProfileModel>>"));
        assert!(generated.contains(
            "pub const profile: Relation<Self, Profile, OneToOne> = \
Relation::new(Self::id, Profile::user_id);"
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

        let generated = generate(&schema).unwrap();

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

        let generated = generate(&schema).unwrap();

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
    #[test]
    fn generates_insert_data() {
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
                    Field {
                        name: "age".to_string(),
                        field_type: FieldType::Int,
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
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema).unwrap();

        assert!(generated.contains("impl rustorm::executor::InsertData<User> for UserCreate"));

        assert!(generated.contains("&[\"name\", \"age\", \"email\", \"active\", \"score\"]"));

        assert!(generated.contains("rustorm::value::BindValue::String((&self.name).clone())"));

        assert!(generated.contains("rustorm::value::BindValue::I64(i64::from(*(&self.age)))"));

        assert!(generated.contains("rustorm::value::BindValue::Null"));

        assert!(generated.contains("rustorm::value::BindValue::Boolean(*(&self.active))"));

        assert!(generated.contains("rustorm::value::BindValue::F64(*(&self.score))"));

        let insert_start = generated
            .find("impl rustorm::executor::InsertData<User> for UserCreate")
            .expect("InsertData implementation should be generated");

        let insert_end = generated[insert_start..]
            .find("\n}\n")
            .map(|offset| insert_start + offset + 3)
            .expect("InsertData implementation should close");

        let insert_section = &generated[insert_start..insert_end];

        assert!(insert_section.contains("&[\"name\", \"age\", \"email\", \"active\", \"score\"]"));

        assert!(!insert_section.contains("&[\"id\""));
        println!("{generated}");
    }
    #[test]
    fn rejects_scalar_array_fields_for_crud_generation() {
        let schema = Schema {
            models: vec![Model {
                name: "User".to_string(),
                fields: vec![
                    crate::ast::Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    },
                    crate::ast::Field {
                        name: "tags".to_string(),
                        field_type: FieldType::String,
                        nullable: false,
                        is_array: true,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let result = generate(&schema);

        assert_eq!(
            result,
            Err(GenerateError::UnsupportedArrayField {
                model: "User".to_string(),
                field: "tags".to_string(),
            })
        );
    }
    #[test]
    fn generates_update_data() {
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
                    Field {
                        name: "age".to_string(),
                        field_type: FieldType::Int,
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
                    Field {
                        name: "active".to_string(),
                        field_type: FieldType::Boolean,
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        let generated = generate(&schema).unwrap();
        println!("{generated}");
        assert!(generated.contains("pub struct UserUpdate {"));

        assert!(generated.contains("pub name: Option<String>,"));

        assert!(generated.contains("pub age: Option<i32>,"));

        assert!(generated.contains("pub email: Option<Option<String>>,"));

        assert!(generated.contains("pub active: Option<bool>,"));

        assert!(generated.contains("impl rustorm::executor::UpdateData<User> for UserUpdate"));

        assert!(generated.contains("if self.name.is_some()"));

        assert!(generated.contains("if self.email.is_some()"));

        assert!(generated.contains("rustorm::value::BindValue::Null"));
    }
    #[test]
    fn generates_snake_case_relation_loader_for_camel_case_field() {
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
                            name: "userPosts".to_string(),
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
                            attributes: vec![FieldAttribute::Relation(RelationAttribute {
                                name: None,
                                fields: vec!["userId".to_string()],
                                references: vec!["id".to_string()],
                                through: None,
                                pivot_from: None,
                                pivot_to: None,
                                on_delete: None,
                            })],
                        },
                    ],
                    attributes: vec![],
                },
            ],
        };

        let generated = generate(&schema).unwrap();

        assert!(generated.contains("pub user_posts: Vec<PostModel>"));
        assert!(generated.contains("self.user_posts = related;"));
        assert!(!generated.contains("self.userPosts = related;"));
    }
}
