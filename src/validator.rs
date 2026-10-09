use crate::ast::Schema;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    DuplicateModel(String),
    DuplicateField {
        model: String,
        field: String,
    },
    UnknownModel {
        model: String,
        referenced_model: String,
        field: String,
    },
    UnknownRelationField {
        model: String,
        field: String,
        relation_field: String,
    },
    UnknownRelationReference {
        model: String,
        field: String,
        referenced_model: String,
        reference_field: String,
    },
    RelationFieldReferenceLengthMismatch {
        model: String,
        field: String,
        fields_count: usize,
        references_count: usize,
    },
    MissingPrimaryKey {
        model: String,
    },
    MultiplePrimaryKeys {
        model: String,
    },
    IncompleteManyToManyRelation {
        model: String,
        field: String,
    },
    MixedRelationMetadata {
        model: String,
        field: String,
    },
    RelationOnNonModelField {
        model: String,
        field: String,
    },
}

pub fn validate(schema: &Schema) -> Result<(), ValidationError> {
    let mut model_names = HashSet::new();

    // 1. Validate model names and field names
    for model in &schema.models {
        if !model_names.insert(&model.name) {
            return Err(ValidationError::DuplicateModel(model.name.clone()));
        }

        let mut field_names = HashSet::new();

        for field in &model.fields {
            if !field_names.insert(&field.name) {
                return Err(ValidationError::DuplicateField {
                    model: model.name.clone(),
                    field: field.name.clone(),
                });
            }
        }

        let primary_key_count = model
            .fields
            .iter()
            .filter(|field| {
                field
                    .attributes
                    .iter()
                    .any(|attribute| matches!(attribute, crate::ast::FieldAttribute::Id))
            })
            .count();

        match primary_key_count {
            0 => {
                return Err(ValidationError::MissingPrimaryKey {
                    model: model.name.clone(),
                });
            }
            1 => {}
            _ => {
                return Err(ValidationError::MultiplePrimaryKeys {
                    model: model.name.clone(),
                });
            }
        }
    }

    // 2. Validate model references
    for model in &schema.models {
        for field in &model.fields {
            if let crate::ast::FieldType::Model(referenced_model) = &field.field_type
                && !model_names.contains(referenced_model)
            {
                return Err(ValidationError::UnknownModel {
                    model: model.name.clone(),
                    referenced_model: referenced_model.clone(),
                    field: field.name.clone(),
                });
            }
        }
    }

    // 3. Validate relation fields and references
    // 3. Validate relation fields and references
    for model in &schema.models {
        for field in &model.fields {
            for attribute in &field.attributes {
                let crate::ast::FieldAttribute::Relation(relation) = attribute else {
                    continue;
                };
                if !matches!(&field.field_type, crate::ast::FieldType::Model(_)) {
                    return Err(ValidationError::RelationOnNonModelField {
                        model: model.name.clone(),
                        field: field.name.clone(),
                    });
                }

                let has_pivot_metadata = relation.through.is_some()
                    || relation.pivot_from.is_some()
                    || relation.pivot_to.is_some();

                let has_normal_metadata =
                    !relation.fields.is_empty() || !relation.references.is_empty();
               
                // Many-to-Many relation
                if has_pivot_metadata {
                    // A Many-to-Many relation must have all pivot
                    // metadata and must not mix it with normal FK metadata.
                    if relation.through.is_none()
                        || relation.pivot_from.is_none()
                        || relation.pivot_to.is_none()
                    {
                        return Err(ValidationError::IncompleteManyToManyRelation {
                            model: model.name.clone(),
                            field: field.name.clone(),
                        });
                    }

                    if has_normal_metadata {
                        return Err(ValidationError::MixedRelationMetadata {
                            model: model.name.clone(),
                            field: field.name.clone(),
                        });
                    }

                    continue;
                }

                // Normal relation
                if relation.fields.len() != relation.references.len() {
                    return Err(ValidationError::RelationFieldReferenceLengthMismatch {
                        model: model.name.clone(),
                        field: field.name.clone(),
                        fields_count: relation.fields.len(),
                        references_count: relation.references.len(),
                    });
                }

                for relation_field in &relation.fields {
                    if !model.fields.iter().any(|f| f.name == *relation_field) {
                        return Err(ValidationError::UnknownRelationField {
                            model: model.name.clone(),
                            field: field.name.clone(),
                            relation_field: relation_field.clone(),
                        });
                    }
                }

                let crate::ast::FieldType::Model(referenced_model) = &field.field_type else {
                    continue;
                };

                let target_model = schema
                    .models
                    .iter()
                    .find(|m| m.name == *referenced_model)
                    .expect("referenced model was already validated");

                for reference_field in &relation.references {
                    if !target_model
                        .fields
                        .iter()
                        .any(|f| f.name == *reference_field)
                    {
                        return Err(ValidationError::UnknownRelationReference {
                            model: model.name.clone(),
                            field: field.name.clone(),
                            referenced_model: referenced_model.clone(),
                            reference_field: reference_field.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Field, FieldAttribute, FieldType, Model, RelationAttribute, Schema};

    #[test]
    fn rejects_duplicate_models() {
        let schema = Schema {
            models: vec![
                Model {
                    name: "User".to_string(),
                    fields: vec![Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    }],
                    attributes: vec![],
                },
                Model {
                    name: "User".to_string(),
                    fields: vec![],
                    attributes: vec![],
                },
            ],
        };

        assert_eq!(
            validate(&schema),
            Err(ValidationError::DuplicateModel("User".to_string()))
        );
    }

    #[test]
    fn accepts_unique_models() {
        let schema = Schema {
            models: vec![
                Model {
                    name: "User".to_string(),
                    fields: vec![Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    }],
                    attributes: vec![],
                },
                Model {
                    name: "Post".to_string(),
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

        assert_eq!(validate(&schema), Ok(()));
    }
    #[test]
    fn rejects_duplicate_fields() {
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

        assert_eq!(
            validate(&schema),
            Err(ValidationError::DuplicateField {
                model: "User".to_string(),
                field: "name".to_string(),
            })
        );
    }
    #[test]
    fn rejects_unknown_model_reference() {
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
                        name: "author".to_string(),
                        field_type: FieldType::Model("User".to_string()),
                        nullable: false,
                        is_array: false,
                        attributes: vec![],
                    },
                ],
                attributes: vec![],
            }],
        };

        assert_eq!(
            validate(&schema),
            Err(ValidationError::UnknownModel {
                model: "Post".to_string(),
                referenced_model: "User".to_string(),
                field: "author".to_string(),
            })
        );
    }
    #[test]
    fn accepts_existing_model_reference() {
        let schema = Schema {
            models: vec![
                Model {
                    name: "User".to_string(),
                    fields: vec![Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    }],
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
                            name: "author".to_string(),
                            field_type: FieldType::Model("User".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![],
                        },
                    ],
                    attributes: vec![],
                },
            ],
        };

        assert_eq!(validate(&schema), Ok(()));
    }
    #[test]
    fn rejects_unknown_relation_field() {
        let schema = Schema {
            models: vec![
                Model {
                    name: "User".to_string(),
                    fields: vec![Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    }],
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
                            name: "author".to_string(),
                            field_type: FieldType::Model("User".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Relation(RelationAttribute {
                                name: None,
                                fields: vec!["authorId".to_string()],
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

        assert_eq!(
            validate(&schema),
            Err(ValidationError::UnknownRelationField {
                model: "Post".to_string(),
                field: "author".to_string(),
                relation_field: "authorId".to_string(),
            })
        );
    }
    #[test]
    fn rejects_unknown_relation_reference() {
        let schema = Schema {
            models: vec![
                Model {
                    name: "User".to_string(),
                    fields: vec![Field {
                        name: "id".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    }],
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
                            name: "authorId".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![],
                        },
                        Field {
                            name: "author".to_string(),
                            field_type: FieldType::Model("User".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Relation(RelationAttribute {
                                name: None,
                                fields: vec!["authorId".to_string()],
                                references: vec!["missingId".to_string()],
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

        assert_eq!(
            validate(&schema),
            Err(ValidationError::UnknownRelationReference {
                model: "Post".to_string(),
                field: "author".to_string(),
                referenced_model: "User".to_string(),
                reference_field: "missingId".to_string(),
            })
        );
    }
    #[test]
    fn rejects_relation_field_reference_length_mismatch() {
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
                            name: "tenantId".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
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
                            name: "authorId".to_string(),
                            field_type: FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![],
                        },
                        Field {
                            name: "author".to_string(),
                            field_type: FieldType::Model("User".to_string()),
                            nullable: false,
                            is_array: false,
                            attributes: vec![FieldAttribute::Relation(RelationAttribute {
                                name: None,
                                fields: vec!["authorId".to_string()],
                                references: vec!["id".to_string(), "tenantId".to_string()],
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

        assert_eq!(
            validate(&schema),
            Err(ValidationError::RelationFieldReferenceLengthMismatch {
                model: "Post".to_string(),
                field: "author".to_string(),
                fields_count: 1,
                references_count: 2,
            })
        );
    }
    #[test]
    fn rejects_model_without_primary_key() {
        let schema = Schema {
            models: vec![Model {
                name: "User".to_string(),
                fields: vec![Field {
                    name: "name".to_string(),
                    field_type: FieldType::String,
                    nullable: false,
                    is_array: false,
                    attributes: vec![],
                }],
                attributes: vec![],
            }],
        };

        assert_eq!(
            validate(&schema),
            Err(ValidationError::MissingPrimaryKey {
                model: "User".to_string(),
            })
        );
    }
    #[test]
    fn rejects_multiple_primary_keys() {
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
                        name: "otherId".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Id],
                    },
                ],
                attributes: vec![],
            }],
        };

        assert_eq!(
            validate(&schema),
            Err(ValidationError::MultiplePrimaryKeys {
                model: "User".to_string(),
            })
        );
    }
    #[test]
    fn accepts_many_to_many_relation() {
        let schema = Schema {
            models: vec![
                crate::ast::Model {
                    name: "User".to_string(),
                    fields: vec![
                        crate::ast::Field {
                            name: "id".to_string(),
                            field_type: crate::ast::FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![crate::ast::FieldAttribute::Id],
                        },
                        crate::ast::Field {
                            name: "roles".to_string(),
                            field_type: crate::ast::FieldType::Model("Role".to_string()),
                            nullable: false,
                            is_array: true,
                            attributes: vec![crate::ast::FieldAttribute::Relation(
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
                crate::ast::Model {
                    name: "Role".to_string(),
                    fields: vec![crate::ast::Field {
                        name: "id".to_string(),
                        field_type: crate::ast::FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![crate::ast::FieldAttribute::Id],
                    }],
                    attributes: vec![],
                },
            ],
        };

        assert!(validate(&schema).is_ok());
    }
    #[test]
    fn rejects_incomplete_many_to_many_relation() {
        let schema = Schema {
            models: vec![
                crate::ast::Model {
                    name: "User".to_string(),
                    fields: vec![
                        crate::ast::Field {
                            name: "id".to_string(),
                            field_type: crate::ast::FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![crate::ast::FieldAttribute::Id],
                        },
                        crate::ast::Field {
                            name: "roles".to_string(),
                            field_type: crate::ast::FieldType::Model("Role".to_string()),
                            nullable: false,
                            is_array: true,
                            attributes: vec![crate::ast::FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    name: Some("UserRoles".to_string()),
                                    fields: vec![],
                                    references: vec![],
                                    through: Some("user_roles".to_string()),
                                    pivot_from: Some("user_id".to_string()),
                                    pivot_to: None,
                                    on_delete: None,
                                },
                            )],
                        },
                    ],
                    attributes: vec![],
                },
                crate::ast::Model {
                    name: "Role".to_string(),
                    fields: vec![crate::ast::Field {
                        name: "id".to_string(),
                        field_type: crate::ast::FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![crate::ast::FieldAttribute::Id],
                    }],
                    attributes: vec![],
                },
            ],
        };

        assert_eq!(
            validate(&schema),
            Err(ValidationError::IncompleteManyToManyRelation {
                model: "User".to_string(),
                field: "roles".to_string(),
            })
        );
    }
    #[test]
    fn rejects_mixed_relation_metadata() {
        let schema = Schema {
            models: vec![
                crate::ast::Model {
                    name: "User".to_string(),
                    fields: vec![
                        crate::ast::Field {
                            name: "id".to_string(),
                            field_type: crate::ast::FieldType::Int,
                            nullable: false,
                            is_array: false,
                            attributes: vec![crate::ast::FieldAttribute::Id],
                        },
                        crate::ast::Field {
                            name: "roles".to_string(),
                            field_type: crate::ast::FieldType::Model("Role".to_string()),
                            nullable: false,
                            is_array: true,
                            attributes: vec![crate::ast::FieldAttribute::Relation(
                                crate::ast::RelationAttribute {
                                    name: Some("UserRoles".to_string()),
                                    fields: vec!["id".to_string()],
                                    references: vec!["id".to_string()],
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
                crate::ast::Model {
                    name: "Role".to_string(),
                    fields: vec![crate::ast::Field {
                        name: "id".to_string(),
                        field_type: crate::ast::FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![crate::ast::FieldAttribute::Id],
                    }],
                    attributes: vec![],
                },
            ],
        };

        assert_eq!(
            validate(&schema),
            Err(ValidationError::MixedRelationMetadata {
                model: "User".to_string(),
                field: "roles".to_string(),
            })
        );
    }
    #[test]
    fn rejects_relation_on_non_model_field() {
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
                        name: "age".to_string(),
                        field_type: FieldType::Int,
                        nullable: false,
                        is_array: false,
                        attributes: vec![FieldAttribute::Relation(RelationAttribute {
                            name: None,
                            fields: vec![],
                            references: vec![],
                            through: None,
                            pivot_from: None,
                            pivot_to: None,
                            on_delete: None,
                        })],
                    },
                ],
                attributes: vec![],
            }],
        };

        assert_eq!(
            validate(&schema),
            Err(ValidationError::RelationOnNonModelField {
                model: "User".to_string(),
                field: "age".to_string(),
            })
        );
    }
}
