use crate::ast::{
    Field, FieldAttribute, FieldType, Model, ModelAttribute, ReferentialAction, RelationAttribute,
    Schema,
};
use crate::lexer::Token;

#[derive(Debug, Clone, PartialEq)]
pub enum ParserError {
    UnexpectedEnd,
    UnexpectedToken(Token),
    ExpectedIdentifier,
    ExpectedToken {
        expected: &'static str,
        found: Option<Token>,
    },
}

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }
    fn parse_relation_attribute(&mut self) -> Result<RelationAttribute, ParserError> {
        self.expect(Token::LeftParen)?;

        let mut name = None;
        let mut fields = Vec::new();
        let mut references = Vec::new();
        let mut on_delete = None;
        let mut through = None;
        let mut pivot_from = None;
        let mut pivot_to = None;

        while !self.check(&Token::RightParen) {
            let key = self.expect_identifier_value()?;

            self.expect(Token::Colon)?;

            match key.as_str() {
                "name" => {
                    name = Some(match self.advance() {
                        Some(Token::String(value)) => value,
                        Some(token) => return Err(ParserError::UnexpectedToken(token)),
                        None => return Err(ParserError::UnexpectedEnd),
                    });
                }

                "fields" => {
                    fields = self.parse_field_name_list()?;
                }

                "references" => {
                    references = self.parse_field_name_list()?;
                }

                "onDelete" => {
                    let value = self.expect_identifier_value()?;

                    on_delete = Some(match value.as_str() {
                        "Cascade" => ReferentialAction::Cascade,
                        "Restrict" => ReferentialAction::Restrict,
                        "SetNull" => ReferentialAction::SetNull,
                        "NoAction" => ReferentialAction::NoAction,
                        _ => {
                            return Err(ParserError::UnexpectedToken(Token::Identifier(value)));
                        }
                    });
                }
                "through" => {
                    through = Some(match self.advance() {
                        Some(Token::String(value)) => value,
                        Some(Token::Identifier(value)) => value,
                        Some(token) => return Err(ParserError::UnexpectedToken(token)),
                        None => return Err(ParserError::UnexpectedEnd),
                    });
                }
                "pivotFrom" => {
                    pivot_from = Some(match self.advance() {
                        Some(Token::String(value)) => value,
                        Some(Token::Identifier(value)) => value,
                        Some(token) => return Err(ParserError::UnexpectedToken(token)),
                        None => return Err(ParserError::UnexpectedEnd),
                    });
                }
                "pivotTo" => {
                    pivot_to = Some(match self.advance() {
                        Some(Token::String(value)) => value,
                        Some(Token::Identifier(value)) => value,
                        Some(token) => return Err(ParserError::UnexpectedToken(token)),
                        None => return Err(ParserError::UnexpectedEnd),
                    });
                }

                _ => {
                    return Err(ParserError::UnexpectedToken(Token::Identifier(key)));
                }
            }

            if !self.consume(&Token::Comma) {
                break;
            }
        }

        self.expect(Token::RightParen)?;

        Ok(RelationAttribute {
            name,
            fields,
            references,
            through,
            pivot_from,
            pivot_to,
            on_delete,
        })
    }

    pub fn parse(mut self) -> Result<Schema, ParserError> {
        let mut models = Vec::new();

        while self.peek().is_some() {
            models.push(self.parse_model()?);
        }

        Ok(Schema { models })
    }
    fn parse_field_name_list(&mut self) -> Result<Vec<String>, ParserError> {
        self.expect(Token::LeftBracket)?;

        let mut fields = Vec::new();

        while !self.check(&Token::RightBracket) {
            let field = self.expect_identifier_value()?;
            fields.push(field);

            if !self.consume(&Token::Comma) {
                break;
            }
        }

        self.expect(Token::RightBracket)?;

        Ok(fields)
    }
    fn parse_model_attribute(&mut self) -> Result<ModelAttribute, ParserError> {
        let name = self.expect_identifier_value()?;

        self.expect(Token::LeftParen)?;

        let attribute = match name.as_str() {
            "map" => {
                let value = match self.advance() {
                    Some(Token::String(value)) => value,
                    Some(token) => return Err(ParserError::UnexpectedToken(token)),
                    None => return Err(ParserError::UnexpectedEnd),
                };

                ModelAttribute::Map(value)
            }

            "index" => {
                let fields = self.parse_field_name_list()?;
                ModelAttribute::Index(fields)
            }

            "unique" => {
                let fields = self.parse_field_name_list()?;
                ModelAttribute::Unique(fields)
            }

            _ => {
                return Err(ParserError::UnexpectedToken(Token::Identifier(name)));
            }
        };

        self.expect(Token::RightParen)?;

        Ok(attribute)
    }
    fn parse_model(&mut self) -> Result<Model, ParserError> {
        self.expect_identifier("model")?;

        let name = self.expect_identifier_value()?;

        self.expect(Token::LeftBrace)?;

        let mut fields = Vec::new();
        let mut attributes = Vec::new();

        while !self.check(&Token::RightBrace) {
            if self.peek().is_none() {
                return Err(ParserError::UnexpectedEnd);
            }

            if self.check(&Token::At) {
                self.expect(Token::At)?;

                if self.consume(&Token::At) {
                    attributes.push(self.parse_model_attribute()?);
                } else {
                    return Err(ParserError::UnexpectedToken(Token::At));
                }
            } else {
                fields.push(self.parse_field()?);
            }
        }

        self.expect(Token::RightBrace)?;

        Ok(Model {
            name,
            fields,
            attributes,
        })
    }

    fn parse_field(&mut self) -> Result<Field, ParserError> {
        let name = self.expect_identifier_value()?;

        let type_name = self.expect_identifier_value()?;

        let field_type = match type_name.as_str() {
            "Int" => FieldType::Int,
            "String" => FieldType::String,
            "Boolean" => FieldType::Boolean,
            "Float" => FieldType::Float,
            "DateTime" => FieldType::DateTime,
            "Decimal" => FieldType::Decimal,
            "Json" => FieldType::Json,
            other => FieldType::Model(other.to_string()),
        };

        let nullable = self.consume(&Token::Question);

        let is_array = if self.consume(&Token::LeftBracket) {
            self.expect(Token::RightBracket)?;
            true
        } else {
            false
        };

        let attributes = self.parse_field_attributes()?;

        Ok(Field {
            name,
            field_type,
            nullable,
            is_array,
            attributes,
        })
    }

    fn parse_default_value(&mut self) -> Result<crate::ast::DefaultValue, ParserError> {
        use crate::ast::DefaultValue;

        match self.advance() {
            Some(Token::Identifier(value)) => match value.as_str() {
                "autoincrement" => {
                    self.expect(Token::LeftParen)?;
                    self.expect(Token::RightParen)?;
                    Ok(DefaultValue::AutoIncrement)
                }

                "now" => {
                    self.expect(Token::LeftParen)?;
                    self.expect(Token::RightParen)?;
                    Ok(DefaultValue::Now)
                }

                "cuid" => {
                    self.expect(Token::LeftParen)?;
                    self.expect(Token::RightParen)?;
                    Ok(DefaultValue::Cuid)
                }

                "true" => Ok(DefaultValue::Boolean(true)),

                "false" => Ok(DefaultValue::Boolean(false)),

                other => Ok(DefaultValue::Identifier(other.to_string())),
            },

            Some(Token::String(value)) => Ok(DefaultValue::String(value)),

            Some(Token::Number(value)) => {
                if value.contains('.') {
                    let number = value
                        .parse::<f64>()
                        .map_err(|_| ParserError::UnexpectedToken(Token::Number(value.clone())))?;

                    Ok(DefaultValue::Float(number))
                } else {
                    let number = value
                        .parse::<i64>()
                        .map_err(|_| ParserError::UnexpectedToken(Token::Number(value.clone())))?;

                    Ok(DefaultValue::Integer(number))
                }
            }

            Some(token) => Err(ParserError::UnexpectedToken(token)),

            None => Err(ParserError::UnexpectedEnd),
        }
    }
    fn parse_field_attributes(&mut self) -> Result<Vec<FieldAttribute>, ParserError> {
        let mut attributes = Vec::new();

        while self.check(&Token::At) && self.peek_next() != Some(&Token::At) {
            self.advance();
            let name = self.expect_identifier_value()?;

            let attribute = match name.as_str() {
                "id" => FieldAttribute::Id,

                "unique" => FieldAttribute::Unique,

                "updatedAt" => FieldAttribute::UpdatedAt,

                "default" => {
                    self.expect(Token::LeftParen)?;
                    let value = self.parse_default_value()?;
                    self.expect(Token::RightParen)?;

                    FieldAttribute::Default(value)
                }

                "map" => {
                    self.expect(Token::LeftParen)?;

                    let value = match self.advance() {
                        Some(Token::String(value)) => value,
                        Some(token) => return Err(ParserError::UnexpectedToken(token)),
                        None => return Err(ParserError::UnexpectedEnd),
                    };

                    self.expect(Token::RightParen)?;

                    FieldAttribute::Map(value)
                }
                "relation" => {
                    let relation = self.parse_relation_attribute()?;
                    FieldAttribute::Relation(relation)
                }

                _ => {
                    return Err(ParserError::UnexpectedToken(Token::Identifier(name)));
                }
            };

            attributes.push(attribute);
        }

        Ok(attributes)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }
    fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.position + 1)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();

        if token.is_some() {
            self.position += 1;
        }

        token
    }

    fn check(&self, expected: &Token) -> bool {
        self.peek() == Some(expected)
    }

    fn consume(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParserError> {
        match self.advance() {
            Some(token) if token == expected => Ok(()),

            found => Err(ParserError::ExpectedToken {
                expected: token_name(&expected),
                found,
            }),
        }
    }

    fn expect_identifier(&mut self, expected: &str) -> Result<(), ParserError> {
        match self.advance() {
            Some(Token::Identifier(value)) if value == expected => Ok(()),

            Some(token) => Err(ParserError::UnexpectedToken(token)),

            None => Err(ParserError::UnexpectedEnd),
        }
    }

    fn expect_identifier_value(&mut self) -> Result<String, ParserError> {
        match self.advance() {
            Some(Token::Identifier(value)) => Ok(value),
            Some(_) => Err(ParserError::ExpectedIdentifier),
            None => Err(ParserError::UnexpectedEnd),
        }
    }
}

fn token_name(token: &Token) -> &'static str {
    match token {
        Token::Identifier(_) => "identifier",
        Token::String(_) => "string",
        Token::Number(_) => "number",
        Token::At => "@",
        Token::Question => "?",
        Token::Comma => ",",
        Token::LeftBrace => "{",
        Token::RightBrace => "}",
        Token::LeftBracket => "[",
        Token::RightBracket => "]",
        Token::LeftParen => "(",
        Token::RightParen => ")",
        Token::Colon => ":",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ast::DefaultValue, lexer::tokenize};

    #[test]
    fn parses_basic_model() {
        let input = r#"
            model User {
                id Int @id
                name String
            }
        "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        assert_eq!(schema.models.len(), 1);

        let user = &schema.models[0];

        assert_eq!(user.name, "User");
        assert_eq!(user.fields.len(), 2);

        assert_eq!(user.fields[0].name, "id");
        assert_eq!(user.fields[0].field_type, FieldType::Int);
        assert!(!user.fields[0].nullable);
        assert!(!user.fields[0].is_array);
        assert_eq!(user.fields[0].attributes, vec![FieldAttribute::Id]);

        assert_eq!(user.fields[1].name, "name");
        assert_eq!(user.fields[1].field_type, FieldType::String);
    }

    #[test]
    fn parses_nullable_and_array_fields() {
        let input = r#"
            model Post {
                title String
                description String?
                tags String[]
            }
        "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        let post = &schema.models[0];

        assert_eq!(post.fields.len(), 3);

        assert!(!post.fields[0].nullable);
        assert!(!post.fields[0].is_array);

        assert!(post.fields[1].nullable);
        assert!(!post.fields[1].is_array);

        assert!(!post.fields[2].nullable);
        assert!(post.fields[2].is_array);
    }

    #[test]
    fn parses_multiple_models() {
        let input = r#"
            model User {
                id Int
            }

            model Post {
                id Int
                title String
            }
        "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        assert_eq!(schema.models.len(), 2);
        assert_eq!(schema.models[0].name, "User");
        assert_eq!(schema.models[1].name, "Post");
    }
    #[test]
    fn parses_default_attributes() {
        let input = r#"
        model User {
            id Int @id @default(autoincrement())
            createdAt DateTime @default(now())
            code String @default(cuid())
            active Boolean @default(true)
            score Float @default(1.5)
            age Int @default(18)
            name String @default("Fakhir")
        }
    "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        let user = &schema.models[0];

        assert_eq!(
            user.fields[0].attributes,
            vec![
                FieldAttribute::Id,
                FieldAttribute::Default(DefaultValue::AutoIncrement),
            ]
        );

        assert_eq!(
            user.fields[1].attributes,
            vec![FieldAttribute::Default(DefaultValue::Now)]
        );

        assert_eq!(
            user.fields[2].attributes,
            vec![FieldAttribute::Default(DefaultValue::Cuid)]
        );

        assert_eq!(
            user.fields[3].attributes,
            vec![FieldAttribute::Default(DefaultValue::Boolean(true))]
        );

        assert_eq!(
            user.fields[4].attributes,
            vec![FieldAttribute::Default(DefaultValue::Float(1.5))]
        );

        assert_eq!(
            user.fields[5].attributes,
            vec![FieldAttribute::Default(DefaultValue::Integer(18))]
        );

        assert_eq!(
            user.fields[6].attributes,
            vec![FieldAttribute::Default(DefaultValue::String(
                "Fakhir".to_string()
            ))]
        );
    }
    #[test]
    fn parses_map_attribute() {
        let input = r#"
        model User {
            id Int @id
            username String @map("user_name")
        }
    "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        let user = &schema.models[0];

        assert_eq!(
            user.fields[1].attributes,
            vec![FieldAttribute::Map("user_name".to_string())]
        );
    }
    #[test]
    fn parses_model_attributes() {
        let input = r#"
        model Wishlist {
            userId Int
            productId Int

            @@map("wishlists")
            @@index([userId])
            @@unique([userId, productId])
        }
    "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        let wishlist = &schema.models[0];

        assert_eq!(
            wishlist.attributes,
            vec![
                ModelAttribute::Map("wishlists".to_string()),
                ModelAttribute::Index(vec!["userId".to_string()]),
                ModelAttribute::Unique(vec!["userId".to_string(), "productId".to_string(),]),
            ]
        );
    }
    #[test]
    fn parses_relation_attribute() {
        let input = r#"
        model Post {
            id Int @id
            authorId Int

            author User @relation(
                fields: [authorId],
                references: [id]
            )
        }
    "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        let post = &schema.models[0];

        assert_eq!(post.fields.len(), 3);

        assert_eq!(
            post.fields[2].attributes,
            vec![FieldAttribute::Relation(RelationAttribute {
                name: None,
                fields: vec!["authorId".to_string()],
                references: vec!["id".to_string()],
                through: None,
                pivot_from: None,
                pivot_to: None,
                on_delete: None,
            })]
        );
    }
    #[test]
    fn parses_named_relation_with_on_delete() {
        let input = r#"
        model Post {
            id Int @id
            authorId Int

            author User @relation(
                name: "PostAuthor",
                fields: [authorId],
                references: [id],
                onDelete: Cascade
            )
        }
    "#;

        let tokens = tokenize(input).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        let post = &schema.models[0];

        assert_eq!(
            post.fields[2].attributes,
            vec![FieldAttribute::Relation(RelationAttribute {
                name: Some("PostAuthor".to_string()),
                fields: vec!["authorId".to_string()],
                references: vec!["id".to_string()],
                through: None,
                pivot_from: None,
                pivot_to: None,
                on_delete: Some(ReferentialAction::Cascade),
            })]
        );
    }
    #[test]
    fn parses_all_referential_actions() {
        let actions = [
            ("Cascade", ReferentialAction::Cascade),
            ("Restrict", ReferentialAction::Restrict),
            ("SetNull", ReferentialAction::SetNull),
            ("NoAction", ReferentialAction::NoAction),
        ];

        for (name, expected) in actions {
            let input = format!(
                r#"
            model Post {{
                id Int @id
                authorId Int

                author User @relation(
                    fields: [authorId],
                    references: [id],
                    onDelete: {name}
                )
            }}
            "#
            );

            let tokens = tokenize(&input).unwrap();
            let schema = Parser::new(tokens).parse().unwrap();

            let post = &schema.models[0];

            assert_eq!(
                post.fields[2].attributes,
                vec![FieldAttribute::Relation(RelationAttribute {
                    name: None,
                    fields: vec!["authorId".to_string()],
                    references: vec!["id".to_string()],
                    through: None,
                    pivot_from: None,
                    pivot_to: None,
                    on_delete: Some(expected),
                })]
            );
        }
    }
    #[test]
    fn parses_many_to_many_relation() {
        let source = r#"
        model User {
            id Int @id
            roles Role[] @relation(
                name: "UserRoles",
                through: "user_roles",
                pivotFrom: "user_id",
                pivotTo: "role_id"
            )
        }

        model Role {
            id Int @id
            users User[]
        }
    "#;

        let tokens = crate::lexer::tokenize(source).unwrap();
        let schema = Parser::new(tokens).parse().unwrap();

        let user = &schema.models[0];
        let roles = &user.fields[1];

        assert_eq!(
            roles.attributes,
            vec![FieldAttribute::Relation(RelationAttribute {
                name: Some("UserRoles".to_string()),
                fields: vec![],
                references: vec![],
                through: Some("user_roles".to_string()),
                pivot_from: Some("user_id".to_string()),
                pivot_to: Some("role_id".to_string()),
                on_delete: None,
            })]
        );
    }
}
