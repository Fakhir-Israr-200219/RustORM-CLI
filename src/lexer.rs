#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Identifier(String),
    String(String),
    Number(String),

    At,
    Colon,
    LeftBrace,
    Question,
    Comma,
    RightBrace,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LexerError {
    InvalidCharacter(char),
    UnterminatedString,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, LexerError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            c if c.is_whitespace() => {
                chars.next();
            }

            '/' => {
                chars.next();

                if let Some('/') = chars.peek() {
                    while let Some(&next) = chars.peek() {
                        chars.next();

                        if next == '\n' {
                            break;
                        }
                    }
                } else {
                    return Err(LexerError::InvalidCharacter('/'));
                }
            }

            '@' => {
                chars.next();
                tokens.push(Token::At);
            }

            '?' => {
                chars.next();
                tokens.push(Token::Question);
            }

            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }

            '{' => {
                chars.next();
                tokens.push(Token::LeftBrace);
            }

            '}' => {
                chars.next();
                tokens.push(Token::RightBrace);
            }

            '[' => {
                chars.next();
                tokens.push(Token::LeftBracket);
            }

            ']' => {
                chars.next();
                tokens.push(Token::RightBracket);
            }

            '(' => {
                chars.next();
                tokens.push(Token::LeftParen);
            }

            ')' => {
                chars.next();
                tokens.push(Token::RightParen);
            }

            '"' => {
                chars.next();

                let mut value = String::new();
                let mut terminated = false;

                while let Some(&next) = chars.peek() {
                    chars.next();

                    if next == '"' {
                        terminated = true;
                        break;
                    }

                    value.push(next);
                }

                if !terminated {
                    return Err(LexerError::UnterminatedString);
                }

                tokens.push(Token::String(value));
            }

            c if c.is_ascii_digit() || c == '-' => {
                let mut value = String::new();

                while let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() || next == '.' || next == '-' {
                        value.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }

                tokens.push(Token::Number(value));
            }

            c if is_identifier_start(c) => {
                let mut value = String::new();

                while let Some(&next) = chars.peek() {
                    if is_identifier_char(next) {
                        value.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }

                tokens.push(Token::Identifier(value));
            }
            ':' => {
                chars.next();
                tokens.push(Token::Colon);
            }
            _ => {
                return Err(LexerError::InvalidCharacter(ch));
            }
        }
    }

    Ok(tokens)
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_basic_model() {
        let input = r#"
            model User {
                id Int @id @default(autoincrement())
                name String
            }
        "#;

        let tokens = tokenize(input).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Identifier("model".to_string()),
                Token::Identifier("User".to_string()),
                Token::LeftBrace,
                Token::Identifier("id".to_string()),
                Token::Identifier("Int".to_string()),
                Token::At,
                Token::Identifier("id".to_string()),
                Token::At,
                Token::Identifier("default".to_string()),
                Token::LeftParen,
                Token::Identifier("autoincrement".to_string()),
                Token::LeftParen,
                Token::RightParen,
                Token::RightParen,
                Token::Identifier("name".to_string()),
                Token::Identifier("String".to_string()),
                Token::RightBrace,
            ]
        );
    }

    #[test]
    fn tokenizes_nullable_array_and_strings() {
        let input = r#"
            description String?
            tags String[]
            @@map("users")
        "#;

        let tokens = tokenize(input).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Identifier("description".to_string()),
                Token::Identifier("String".to_string()),
                Token::Question,
                Token::Identifier("tags".to_string()),
                Token::Identifier("String".to_string()),
                Token::LeftBracket,
                Token::RightBracket,
                Token::At,
                Token::At,
                Token::Identifier("map".to_string()),
                Token::LeftParen,
                Token::String("users".to_string()),
                Token::RightParen,
            ]
        );
    }

    #[test]
    fn ignores_comments() {
        let input = r#"
            // this is a comment
            model User {
                id Int // inline comment
                name String
            }
        "#;

        let tokens = tokenize(input).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Identifier("model".to_string()),
                Token::Identifier("User".to_string()),
                Token::LeftBrace,
                Token::Identifier("id".to_string()),
                Token::Identifier("Int".to_string()),
                Token::Identifier("name".to_string()),
                Token::Identifier("String".to_string()),
                Token::RightBrace,
            ]
        );
    }

    #[test]
    fn rejects_invalid_character() {
        let result = tokenize("model User { name String $ }");

        assert_eq!(result, Err(LexerError::InvalidCharacter('$')));
    }

    #[test]
    fn rejects_unterminated_string() {
        let result = tokenize(r#"@@map("users)"#);

        assert_eq!(result, Err(LexerError::UnterminatedString));
    }
}
