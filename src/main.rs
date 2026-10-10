mod ast;
mod generator;
mod lexer;
mod migration;
mod parser;
mod validator;

use std::{env, fs, path::Path, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() == 1 {
        match args[0].as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" | "-V" => {
                println!("RustORM-CLI {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }

    if let Some(option) = args.iter().find(|arg| arg.starts_with('-')) {
        return Err(format!(
            "Unknown option '{option}'. Run 'RustORM-CLI --help' for usage."
        ));
    }

    let (command, schema_path, output_path) = match args.first().map(String::as_str) {
        Some("validate") => {
            validate_argument_count(&args, 1, 2, "validate [SCHEMA]")?;
            (
                "validate",
                args.get(1).map(String::as_str).unwrap_or("rust.schema"),
                None,
            )
        }
        Some("generate") => {
            validate_argument_count(&args, 1, 2, "generate [SCHEMA]")?;
            (
                "generate",
                args.get(1).map(String::as_str).unwrap_or("rust.schema"),
                None,
            )
        }
        Some("migrate") => {
            validate_argument_count(&args, 1, 3, "migrate [SCHEMA] [OUTPUT]")?;
            (
                "migrate",
                args.get(1).map(String::as_str).unwrap_or("rust.schema"),
                Some(
                    args.get(2)
                        .map(String::as_str)
                        .unwrap_or("migrations/0001_initial.sql"),
                ),
            )
        }
        Some(other) if Path::new(other).is_file() && args.len() == 1 => ("generate", other, None),
        Some(other) => {
            return Err(format!(
                "Unknown command '{other}'. Run 'rustorm --help' for usage."
            ));
        }
        None => ("generate", "rust.schema", None),
    };

    let source = fs::read_to_string(schema_path)
        .map_err(|error| format!("Failed to read schema '{schema_path}': {error}"))?;

    let tokens = lexer::tokenize(&source).map_err(|error| format!("Lexer error: {error:?}"))?;

    let schema = parser::Parser::new(tokens)
        .parse()
        .map_err(|error| format!("Parser error: {error:?}"))?;

    validator::validate(&schema).map_err(|error| format!("Validation error: {error:?}"))?;

    match command {
        "validate" => {
            println!("Schema is valid: {schema_path}");
        }
        "generate" => {
            let output_path = "generated/entities.rs";

            generator::write_entities(&schema, output_path)
                .map_err(|error| format!("Failed to write generated entities: {error}"))?;

            println!("Schema is valid: {schema_path}");
            println!("Generated entities: {output_path}");
        }
        "migrate" => {
            let output_path =
                output_path.ok_or_else(|| "Missing migration output path".to_string())?;

            let sql = migration::generate_migration(&schema)?;

            if let Some(parent) = Path::new(output_path).parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Failed to create migration directory: {error}"))?;
            }

            fs::write(output_path, sql)
                .map_err(|error| format!("Failed to write migration '{output_path}': {error}"))?;

            println!("Migration generated: {output_path}");
        }
        _ => return Err(format!("Unsupported command '{command}'")),
    }

    Ok(())
}

fn validate_argument_count(
    args: &[String],
    minimum: usize,
    maximum: usize,
    usage: &str,
) -> Result<(), String> {
    if !(minimum..=maximum).contains(&args.len()) {
        return Err(format!(
            "Invalid number of arguments.\nUsage: RustORM-CLI {usage}"
        ));
    }

    Ok(())
}


fn print_help() {
    println!(
        "rustorm {}\n\
         \n\
         Usage:\n\
           rustorm [SCHEMA]\n\
           rustorm validate [SCHEMA]\n\
           rustorm generate [SCHEMA]\n\
           rustorm migrate [SCHEMA] [OUTPUT]\n\
           rustorm --help\n\
           rustorm --version\n\
         \n\
         Commands:\n\
           validate  Validate a schema\n\
           generate  Generate Rust entities (default)\n\
           migrate   Generate initial PostgreSQL migration SQL\n\
         \n\
         Defaults:\n\
           SCHEMA    rust.schema\n\
           OUTPUT    migrations/0001_initial.sql\n",
        env!("CARGO_PKG_VERSION")
    );
}

