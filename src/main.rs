mod ast;
mod error;
mod generator;
mod lexer;
mod migration;
mod naming;
mod parser;
mod validator;

use crate::ast::Schema;
use crate::error::{CliError, Result as CliResult};
use notify::Watcher;
use std::{env, fs, path::Path};

const SCHEMA_TEMPLATE: &str = r#"// RustORM Schema
// Documentation: https://github.com/Fakhir-Israr-200219/RustORM

model User {
    id    Int    @id @default(autoincrement())
    name  String
    email String @unique
}
"#;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> CliResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    // 1. Handle --help / --version
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

    // Only reject unknown options for simple commands (not migrate/init/dev)
    let first_arg = args.first().map(|s| s.as_str()).unwrap_or("");
    if !matches!(first_arg, "migrate" | "init" | "dev") {
        if let Some(option) = args.iter().find(|arg| arg.starts_with('-')) {
            return Err(
                format!("Unknown option '{option}'. Run 'rustorm --help' for usage.").into(),
            );
        }
    }

    // 2. Handle Commands
    let (command, schema_path, _output_path) = match args.first().map(String::as_str) {
        Some("validate") => {
            validate_argument_count(&args, 1, 2, "validate [SCHEMA]")?;
            (
                "validate",
                args.get(1).map(String::as_str).unwrap_or("rust.schema"),
                None::<&str>,
            )
        }
        Some("init") => {
            let rustorm_dir = Path::new(".rustorm");

            if rustorm_dir.exists() {
                println!(".rustorm/ already exists. Skipping init.");
                return Ok(());
            }

            fs::create_dir_all(rustorm_dir.join("generated"))
                .map_err(|e| format!("Failed to create .rustorm/generated/: {e}"))?;
            fs::create_dir_all(rustorm_dir.join("migrations"))
                .map_err(|e| format!("Failed to create .rustorm/migrations/: {e}"))?;

            fs::write(rustorm_dir.join("schema.rustorm"), SCHEMA_TEMPLATE)
                .map_err(|e| format!("Failed to write schema.rustorm: {e}"))?;

            // Write .env
            let env_content = "# Database connection string\n\
                               # ⚠️ Do NOT commit this file to git!\n\
                               DATABASE_URL=postgres://postgres:admin@localhost/rustorm_gen\n";
            fs::write(rustorm_dir.join(".env"), env_content)
                .map_err(|e| format!("Failed to write .env: {e}"))?;

            // Write config.toml
            let config_content = "[database]\n\
                                  url = \"postgres://postgres:admin@localhost/rustorm_gen\"\n";
            fs::write(rustorm_dir.join("config.toml"), config_content)
                .map_err(|e| format!("Failed to write config.toml: {e}"))?;

            // Write .gitignore
            let gitignore_content = ".env\n";
            fs::write(rustorm_dir.join(".gitignore"), gitignore_content)
                .map_err(|e| format!("Failed to write .gitignore: {e}"))?;

            println!("✅ Initialized RustORM project");
            println!();
            println!("  📁 .rustorm/schema.rustorm   ← Edit your schema here");
            println!("  📁 .rustorm/generated/       ← Entities will be generated here");
            println!("  📁 .rustorm/migrations/      ← Migration SQL will be generated here");
            println!("  📁 .rustorm/.env             ← Database URL (edit this)");
            println!("  📁 .rustorm/config.toml      ← Advanced config");
            println!("  📁 .rustorm/.gitignore       ← Protects .env from git");
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
                return Err(CliError::ConfigError(
                    "No .rustorm/schema.rustorm found. Run 'rustorm init' first.".into(),
                ));
            }

            println!("👀 Watching .rustorm/schema.rustorm for changes...");
            println!("   Press Ctrl+C to stop\n");

            // Initial generation
            run_generation(schema_path, entities_output, migrations_dir, snapshot_path)?;

            // Setup file watcher
            let (tx, rx) = std::sync::mpsc::channel();

            let mut watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                    if let Ok(event) = res {
                        if event.kind.is_modify() || event.kind.is_create() {
                            let _ = tx.send(());
                        }
                    }
                })
                .map_err(|e| format!("Failed to create file watcher: {e}"))?;

            watcher
                .watch(schema_path, notify::RecursiveMode::NonRecursive)
                .map_err(|e| format!("Failed to watch schema file: {e}"))?;

            // Watch loop
            loop {
                match rx.recv() {
                    Ok(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        while rx.try_recv().is_ok() {}

                        println!("\n🔄 Schema change detected! Regenerating...");
                        match run_generation(
                            schema_path,
                            entities_output,
                            migrations_dir,
                            snapshot_path,
                        ) {
                            Ok(()) => println!("✅ Regeneration complete\n"),
                            Err(e) => eprintln!("❌ Regeneration failed: {e}\n"),
                        }
                    }
                    Err(_) => break,
                }
            }

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
            validate_argument_count(
                &args,
                1,
                4,
                "migrate [SCHEMA] [OUTPUT] [--apply] [--database URL]",
            )?;

            let has_apply = args.iter().any(|a| a == "--apply");

            if has_apply {
                let db_url_owned = if let Some(pos) = args.iter().position(|a| a == "--database") {
                    args.get(pos + 1)
                        .ok_or_else(|| {
                            CliError::ConfigError("--database requires a connection string".into())
                        })?
                        .clone()
                } else {
                    load_database_url()?
                };
                let db_url: &str = &db_url_owned;

                let schema_path = args
                    .get(1)
                    .map(|s| s.as_str())
                    .filter(|s| !s.starts_with('-'))
                    .unwrap_or(".rustorm/schema.rustorm");

                let migrations_dir = Path::new(schema_path)
                    .parent()
                    .map(|p| p.join("migrations"))
                    .unwrap_or_else(|| Path::new(".rustorm/migrations").to_path_buf());

                println!("🚀 Applying migrations to database...");
                println!("   Database: {}", mask_database_url(db_url));

                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create async runtime: {e}"))?;

                rt.block_on(async {
                    let pool = sqlx::PgPool::connect(db_url)
                        .await
                        .map_err(|e| CliError::ConnectionError(format!("Failed to connect to database: {e}")))?;

                    // Ensure tracking table exists
                    sqlx::raw_sql(
                        "CREATE TABLE IF NOT EXISTS _rustorm_migrations (\n\
                             id SERIAL PRIMARY KEY,\n\
                             name TEXT NOT NULL UNIQUE,\n\
                             checksum TEXT NOT NULL,\n\
                             applied_at TIMESTAMP NOT NULL DEFAULT NOW()\n\
                         );",
                    )
                    .execute(&pool)
                    .await
                    .map_err(|e| CliError::MigrationError(format!("Failed to create migration tracking table: {e}")))?;

                    // Collect all .sql files sorted by name
                    let mut migration_files: Vec<_> = fs::read_dir(&migrations_dir)
                        .map_err(|e| CliError::IoError {
                            path: migrations_dir.clone(),
                            source: e,
                        })?
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| {
                            entry.path().extension()
                                .map(|ext| ext == "sql")
                                .unwrap_or(false)
                        })
                        .collect();

                    migration_files.sort_by_key(|e| e.file_name());

                    if migration_files.is_empty() {
                        return Err(CliError::MigrationError(
                            "No migration files found in .rustorm/migrations/. Run 'rustorm dev' first.".into(),
                        ));
                    }

                    println!("📋 Found {} migration file(s)\n", migration_files.len());

                    for migration_entry in &migration_files {
                        let migration_path = migration_entry.path();
                        let migration_name = migration_path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");

                        let sql = fs::read_to_string(&migration_path)
                            .map_err(|e| CliError::IoError {
                                path: migration_path.clone(),
                                source: e,
                            })?;

                        // Calculate checksum
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        sql.hash(&mut hasher);
                        let checksum = format!("{:x}", hasher.finish());

                        // Check if already applied
                        let existing: Option<(String,)> = sqlx::query_as(
                            "SELECT checksum FROM _rustorm_migrations WHERE name = $1"
                        )
                        .bind(migration_name)
                        .fetch_optional(&pool)
                        .await
                        .map_err(|e| CliError::MigrationError(format!("Failed to check migration history: {e}")))?;

                        if let Some((existing_checksum,)) = existing {
                            if existing_checksum == checksum {
                                println!("⏭️  {} already applied. Skipping.", migration_name);
                                continue;
                            } else {
                                return Err(CliError::MigrationError(format!(
                                    "❌ Schema drift in '{}'! Checksum mismatch.\nOld: {}\nNew: {}",
                                    migration_name, existing_checksum, checksum
                                )));
                            }
                        }

                        // Apply migration
                        println!("📝 Applying: {}", migration_name);
                        sqlx::raw_sql(&sql)
                            .execute(&pool)
                            .await
                            .map_err(|e| CliError::MigrationError(format!("Failed to apply {}: {e}", migration_name)))?;

                        // Record in tracking table
                        sqlx::query(
                            "INSERT INTO _rustorm_migrations (name, checksum) VALUES ($1, $2)"
                        )
                        .bind(migration_name)
                        .bind(&checksum)
                        .execute(&pool)
                        .await
                        .map_err(|e| CliError::MigrationError(format!("Failed to record {}: {e}", migration_name)))?;

                        println!("✅ {} applied successfully!", migration_name);
                    }

                    // List created tables
                    let tables: Vec<(String,)> = sqlx::query_as(
                        "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename"
                    )
                    .fetch_all(&pool)
                    .await
                    .map_err(|e| CliError::MigrationError(format!("Failed to list tables: {e}")))?;

                    println!("\nTables in database:");
                    for (table,) in &tables {
                        println!("  📋 {table}");
                    }

                    pool.close().await;
                    Ok::<(), CliError>(())
                })?;

                return Ok(());
            }

            // Original generate mode (no --apply flag)
            let schema_path = args
                .get(1)
                .map(|s| s.as_str())
                .filter(|s| !s.starts_with('-'))
                .unwrap_or(".rustorm/schema.rustorm");

            let output_path = args
                .get(2)
                .map(|s| s.as_str())
                .filter(|s| !s.starts_with('-'))
                .unwrap_or("migrations/0001_initial.sql");

            let source = fs::read_to_string(schema_path)
                .map_err(|e| format!("Failed to read schema '{schema_path}': {e}"))?;

            let tokens = lexer::tokenize(&source).map_err(|e| format!("Lexer error: {e:?}"))?;

            let schema = parser::Parser::new(tokens)
                .parse()
                .map_err(|e| CliError::ParseError {
                    line: 0,
                    column: 0,
                    message: format!("{e:?}"),
                })?;

            validator::validate(&schema).map_err(|e| format!("Validation error: {e:?}"))?;

            let sql =
                migration::generate_migration(&schema).map_err(|e| CliError::GenerationError(e))?;

            if let Some(parent) = Path::new(output_path).parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create migration directory: {e}"))?;
                }
            }

            fs::write(output_path, sql)
                .map_err(|e| format!("Failed to write migration '{output_path}': {e}"))?;

            println!("Migration generated: {output_path}");
            return Ok(());
        }
        Some(other) if Path::new(other).is_file() && args.len() == 1 => ("generate", other, None),
        Some(other) => {
            return Err(CliError::ConfigError(format!(
                "Unknown command '{other}'. Run 'rustorm --help' for usage."
            )));
        }
        None => ("generate", "rust.schema", None),
    };

    // 3. Parse and Execute (for validate, generate)
    let source = fs::read_to_string(schema_path)
        .map_err(|e| format!("Failed to read schema '{schema_path}': {e}"))?;

    let tokens = lexer::tokenize(&source).map_err(|e| format!("Lexer error: {e:?}"))?;

    let schema = parser::Parser::new(tokens)
        .parse()
        .map_err(|e| CliError::ParseError {
            line: 0,
            column: 0,
            message: format!("{e:?}"),
        })?;

    validator::validate(&schema).map_err(|e| format!("Validation error: {e:?}"))?;

    match command {
        "validate" => {
            println!("Schema is valid: {schema_path}");
        }
        "generate" => {
            let output_path = "generated/entities.rs";

            generator::write_entities(&schema, output_path)
                .map_err(|e| format!("Failed to write generated entities: {e}"))?;

            println!("Schema is valid: {schema_path}");
            println!("Generated entities: {output_path}");
        }
        _ => {
            return Err(CliError::ConfigError(format!(
                "Unsupported command '{command}'"
            )));
        }
    }

    Ok(())
}

fn run_generation(
    schema_path: &Path,
    entities_output: &Path,
    migrations_dir: &Path,
    snapshot_path: &Path,
) -> CliResult<()> {
    let source =
        fs::read_to_string(schema_path).map_err(|e| format!("Failed to read schema: {e}"))?;

    let tokens = lexer::tokenize(&source).map_err(|e| format!("Lexer error: {e:?}"))?;

    let schema = parser::Parser::new(tokens)
        .parse()
        .map_err(|e| CliError::ParseError {
            line: 0,
            column: 0,
            message: format!("{e:?}"),
        })?;

    validator::validate(&schema).map_err(|e| format!("Validation error: {e:?}"))?;

    // Snapshot check
    let current_content = source.clone();
    let schema_changed = if snapshot_path.exists() {
        let old_content = fs::read_to_string(snapshot_path).unwrap_or_default();
        old_content != current_content
    } else {
        true
    };

    if !schema_changed {
        println!("📋 Schema unchanged. Skipping.");
        return Ok(());
    }

    // Generate entities
    fs::create_dir_all(entities_output.parent().unwrap())
        .map_err(|e| format!("Failed to create generated dir: {e}"))?;

    generator::write_entities(&schema, entities_output)
        .map_err(|e| format!("Failed to generate entities: {e}"))?;

    println!("  ✅ Generated: .rustorm/generated/entities.rs");

    // Generate migration
    fs::create_dir_all(migrations_dir)
        .map_err(|e| format!("Failed to create migrations dir: {e}"))?;

    let migration_sql =
        migration::generate_migration(&schema).map_err(|e| CliError::GenerationError(e))?;
    let migration_file = migrations_dir.join("0001_initial.sql");
    fs::write(&migration_file, &migration_sql)
        .map_err(|e| format!("Failed to write migration: {e}"))?;

    println!("  ✅ Generated: .rustorm/migrations/0001_initial.sql");

    // Load old snapshot for diffing
    let old_snapshot: Option<Schema> = if snapshot_path.exists() {
        fs::read_to_string(snapshot_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    } else {
        None
    };

    // Generate diff if old snapshot exists
    if let Some(ref old_snap) = old_snapshot {
        match migration::diff_schema(old_snap, &schema) {
            Ok(diff_sql) => {
                if !diff_sql.trim().is_empty() {
                    // Find next migration number
                    let mut next_num = 1u32;
                    if let Ok(entries) = fs::read_dir(migrations_dir) {
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.ends_with(".sql") {
                                    if let Some(num_str) = name.split('_').next() {
                                        if let Ok(n) = num_str.parse::<u32>() {
                                            next_num = next_num.max(n + 1);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let diff_filename = format!("{:04}_schema_update.sql", next_num);
                    let diff_path = migrations_dir.join(&diff_filename);
                    fs::write(&diff_path, &diff_sql)
                        .map_err(|e| format!("Failed to write diff migration: {e}"))?;
                    println!("  🔄 Schema change detected! Generated: {}", diff_filename);
                }
            }
            Err(e) => {
                println!("  ⚠️  Diff generation failed: {e}");
            }
        }
    }

    // Save new snapshot
    let snapshot_json = serde_json::to_string_pretty(&schema)
        .map_err(|e| format!("Failed to serialize snapshot: {e}"))?;
    fs::write(snapshot_path, &snapshot_json)
        .map_err(|e| format!("Failed to write snapshot: {e}"))?;
    println!("  💾 Snapshot saved");

    Ok(())
}

fn mask_database_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let prefix = &url[..colon_pos + 1];
            let suffix = &url[at_pos..];
            return format!("{prefix}****{suffix}");
        }
    }
    url.to_string()
}

fn load_database_url() -> CliResult<String> {
    // Priority: env var > .env > config.toml
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Ok(url);
    }

    // Try loading .env file
    let env_path = Path::new(".rustorm/.env");
    if env_path.exists() {
        let content =
            fs::read_to_string(env_path).map_err(|e| format!("Failed to read .env: {e}"))?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "DATABASE_URL" {
                    return Ok(value.trim().to_string());
                }
            }
        }
    }

    // Try config.toml
    let config_path = Path::new(".rustorm/config.toml");
    if config_path.exists() {
        let content = fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read config.toml: {e}"))?;
        let parsed: toml::Value = content
            .parse()
            .map_err(|e| format!("Failed to parse config.toml: {e}"))?;
        if let Some(url) = parsed
            .get("database")
            .and_then(|d| d.get("url"))
            .and_then(|u| u.as_str())
        {
            return Ok(url.to_string());
        }
    }

    Err(CliError::ConfigError(
        "No database URL found.\n\
         Create .rustorm/.env with: DATABASE_URL=postgres://...\n\
         Or create .rustorm/config.toml with: [database]\n  url = \"...\""
            .into(),
    ))
}

fn validate_argument_count(args: &[String], min: usize, max: usize, usage: &str) -> CliResult<()> {
    if args.len() < min || args.len() > max {
        return Err(CliError::ConfigError(format!(
            "Invalid arguments. Usage: rustorm {}",
            usage
        )));
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
           rustorm migrate --apply --database URL\n\
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
                     (Use --apply --database URL to execute directly on DB)\n\
         \n\
         Defaults:\n\
           SCHEMA    rust.schema\n\
           OUTPUT    migrations/0001_initial.sql\n",
        env!("CARGO_PKG_VERSION")
    );
}
