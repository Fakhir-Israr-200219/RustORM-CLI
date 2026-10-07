mod ast;
mod generator;
mod lexer;
mod parser;
mod validator;
use std::env;
use std::fs;

fn main() {
    let schema_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "rust.schema".to_string());

    let source = fs::read_to_string(&schema_path).unwrap_or_else(|error| {
        eprintln!("Failed to read {}: {}", schema_path, error);
        std::process::exit(1);
    });

    let tokens = lexer::tokenize(&source).unwrap_or_else(|error| {
        eprintln!("Lexer error: {:?}", error);
        std::process::exit(1);
    });

    let schema = parser::Parser::new(tokens).parse().unwrap_or_else(|error| {
        eprintln!("Parser error: {:?}", error);
        std::process::exit(1);
    });

    validator::validate(&schema).unwrap_or_else(|error| {
        eprintln!("Validation error: {:?}", error);
        std::process::exit(1);
    });
    generator::write_entities(&schema, "generated/entities.rs").unwrap_or_else(|error| {
        eprintln!("Failed to write generated entities: {}", error);
        std::process::exit(1);
    });

    println!("Schema is valid.");
    println!("Generated entities: generated/entities.rs");
}
