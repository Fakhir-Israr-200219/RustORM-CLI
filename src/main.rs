mod ast;
mod generator;
mod lexer;
mod migration;
mod naming;
mod parser;
mod validator;

use std::{env, fs, path::Path, process};

const SCHEMA_TEMPLATE: &str = r#"// RustORM Schema
// Documentation: https://github.com/Fakhir-Israr-200219/RustORM

model User {
    id    Int    @id @default(autoincrement())
    name  String
    email String @unique
}
"#;

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
        Some("init") => {
            let rustorm_dir = Path::new(".rustorm");

            if rustorm_dir.exists() {
                println!(".rustorm/ already exists. Skipping init.");
                return Ok(());
            }

            // Create directories
            fs::create_dir_all(rustorm_dir.join("generated"))
                .map_err(|e| format!("Failed to create .rustorm/generated/: {e}"))?;
            fs::create_dir_all(rustorm_dir.join("migrations"))
                .map_err(|e| format!("Failed to create .rustorm/migrations/: {e}"))?;

            // Write schema template
            fs::write(rustorm_dir.join("schema.rustorm"), SCHEMA_TEMPLATE)
                .map_err(|e| format!("Failed to write schema.rustorm: {e}"))?;

            println!("✅ Initialized RustORM project");
            println!();
            println!("  📁 .rustorm/schema.rustorm   ← Edit your schema here");
            println!("  📁 .rustorm/generated/       ← Entities will be generated here");
            println!("  📁 .rustorm/migrations/      ← Migration SQL will be generated here");
            println!();
            println!("Next steps:");
            println!("  1. Edit .rustorm/schema.rustorm");
            println!("  2. Run: rustorm dev");
            return Ok(());
        }
        Some("dev") => {
            let schema_path = Path::new(".rustorm/schema.rustorm");
            let entities_output = Path::new(".rustorm/generated/entities.rs");
            let migrations_dir = Path::new(".rustorm/migrations");
            let snapshot_path = Path::new(".rustorm/migrations/snapshot.json");

            if !schema_path.exists() {
                return Err("No .rustorm/schema.rustorm found. Run 'rustorm init' first.".into());
            }

            // Read and parse schema
            let source = fs::read_to_string(schema_path)
                .map_err(|e| format!("Failed to read schema: {e}"))?;

            let tokens = lexer::tokenize(&source).map_err(|e| format!("Lexer error: {e:?}"))?;

            let schema = parser::Parser::new(tokens)
                .parse()
                .map_err(|e| format!("Parser error: {e:?}"))?;

            validator::validate(&schema).map_err(|e| format!("Validation error: {e:?}"))?;

            // Check if schema changed (snapshot comparison)
            let current_content = source.clone();
            let schema_changed = if snapshot_path.exists() {
                let old_content = fs::read_to_string(snapshot_path).unwrap_or_default();
                old_content != current_content
            } else {
                true // First run
            };

            if !schema_changed {
                println!("📋 Schema unchanged. Nothing to regenerate.");
                return Ok(());
            }

            // ... (generation code same rahega) ...

            // Save snapshot (raw content, not debug format)
            fs::write(snapshot_path, &current_content)
                .map_err(|e| format!("Failed to save snapshot: {e}"))?;

            generator::write_entities(&schema, entities_output)
                .map_err(|e| format!("Failed to generate entities: {e}"))?;

            println!("✅ Generated: .rustorm/generated/entities.rs");

            // Generate migration
            fs::create_dir_all(migrations_dir)
                .map_err(|e| format!("Failed to create migrations dir: {e}"))?;

            let migration_sql = migration::generate_migration(&schema)?;
            let migration_file = migrations_dir.join("0001_initial.sql");
            fs::write(&migration_file, &migration_sql)
                .map_err(|e| format!("Failed to write migration: {e}"))?;

            println!("✅ Generated: .rustorm/migrations/0001_initial.sql");

            println!("💾 Snapshot saved");
            println!();
            println!("Next: Apply migration with:");
            println!("  psql -U postgres -d YOUR_DB -f .rustorm/migrations/0001_initial.sql");

            return Ok(());
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
           rustorm init\n\
           rustorm dev\n\
           rustorm --help\n\
           rustorm --version\n\
         \n\
         Commands:\n\
           init      Initialize a new RustORM project\n\
           dev       Watch schema and auto-generate\n\
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
