use crate::ast::{Field, FieldAttribute, Model, ModelAttribute};

pub fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            result.push('_');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}

pub fn pluralize(name: &str) -> String {
    format!("{}s", to_snake_case(name))
}

pub fn table_name(model: &Model) -> String {
    model
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            ModelAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| pluralize(&model.name))
}

pub fn column_name(field: &Field) -> String {
    field
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            FieldAttribute::Map(name) => Some(name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| to_snake_case(&field.name))
}

pub fn rust_identifier(name: &str) -> String {
    to_snake_case(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_works() {
        assert_eq!(to_snake_case("userId"), "user_id");
        assert_eq!(to_snake_case("createdAt"), "created_at");
        assert_eq!(to_snake_case("id"), "id");
    }

    #[test]
    fn tables_are_plural_snake_case() {
        assert_eq!(pluralize("User"), "users");
        assert_eq!(pluralize("UserProfile"), "user_profiles");
    }
}