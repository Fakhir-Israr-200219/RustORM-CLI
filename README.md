# RustORM CLI

RustORM CLI is the schema parser, validator, Rust code generator, migration system, and developer workflow tool for RustORM.

It aims to bring a Prisma-inspired development workflow to Rust by using a schema file to describe models, fields, and relations, then generating Rust entity code, managing database migrations with tracking and diffing, and providing watch-mode regeneration with direct database execution.

The CLI is a separate project from the RustORM runtime.

> **Status:** Active development — V0.1  
> Schema parsing, AST construction, semantic validation, entity and CRUD code generation, migration generation with **tracking and schema diffing**, watch-mode development workflow, multi-file migration execution, auto-numbered incremental migrations, `.env`/`config.toml` configuration, **nested eager loading support**, and **Axum integration** are implemented. Generated-code compilation and full application-level integration have been verified end-to-end with an Axum example including **2-level nested eager loading**.

---

## Architecture

```text
rust.schema
    |
    v
Lexer → Parser → Schema AST → Validator
    |
    +--------------------+---------------------+
    |                    |                     |
    v                    v                     v
Entity Generator   Migration Generator   Snapshot Manager
    |                    |                     |
    v                    v                     v
generated/          migrations/           snapshot.json
entities.rs         0001_initial.sql      (change detection)
                    0002_schema_update.sql
                    0003_...
```

## Runtime Boundary

```text
RustORM CLI
    |
    +-- Schema parsing & validation
    +-- Rust code generation (entities, CRUD, serde derives)
    +-- Migration SQL generation (initial + incremental diffs)
    +-- Migration tracking (_rustorm_migrations table)
    +-- Schema diffing (ALTER TABLE ADD/DROP COLUMN)
    +-- Watch-mode regeneration (dev command)
    +-- Multi-file migration execution (--apply flag)
    +-- Configuration (.env / config.toml / env var / CLI arg)
              |
              v
       RustORM Runtime
              |
              +-- Query building
              +-- Database execution
              +-- Entity operations
              +-- Runtime relations
              +-- Nested eager loading (.with / .with_nested)
              +-- Wrapper types (DateTime, Decimal, JsonValue)
              +-- Send + Sync async compatibility
              +-- Transactions
```

The CLI is a toolchain, not a second ORM runtime. Generated Rust code targets the existing RustORM runtime and its supporting dependencies.

The `migrate --apply` command scans all `.sql` files in the migrations directory, checks each against the tracking table via checksum, and applies only pending migrations sequentially.

---

# 1. Schema Language

## Models

Example:

```text
model User {
    id Int @id @default(autoincrement())
    name String
}
```

Models contain fields and may have model-level attributes.

**Status:** Implemented.

## Scalar Types

The parser recognizes these scalar types:

```text
Int
String
Boolean
Float
DateTime
Decimal
Json
```

These types are represented in the Schema AST and mapped to Rust types by the entity generator.

| Schema type | Generated Rust type |
|---|---|
| `Int` | `i32` |
| `String` | `String` |
| `Boolean` | `bool` |
| `Float` | `f64` |
| `DateTime` | `rustorm::DateTime` |
| `Decimal` | `rustorm::Decimal` |
| `Json` | `rustorm::JsonValue` |

Nullable fields use `Option<T>`. Scalar arrays are recognized by the parser and AST, but the current CRUD generator rejects scalar-array fields. Therefore, parser support does not imply complete end-to-end support.

## Nullable Fields

```text
model User {
    id Int @id @default(autoincrement())
    email String?
}
```

Nullable fields are represented in the AST and mapped to `Option<T>` in generated model types.

**Status:** Implemented, subject to the limitations of downstream code generation.

## Array Fields

```text
tags String[]
```

Array syntax is recognized by the parser.

Model-relation arrays are used for collection relations, for example:

```text
posts Post[]
```

**Limitation:** Scalar arrays such as `String[]` are rejected by the current CRUD code-generation validation. Do not assume that every parsed array type is supported by generated CRUD code or migration SQL.

## Field Attributes

The parser recognizes:

```text
@id
@unique
@default(...)
@map(...)
@updatedAt
@relation(...)
```

Default expressions recognized by the parser include:

```text
autoincrement()
now()
cuid()
true
false
integer values
floating-point values
string values
identifier values
```

**Important:** Parsing an attribute does not guarantee that every use is semantically validated or fully implemented by both generators. In particular, `@updatedAt` does not currently have a demonstrated automatic update behavior in the inspected code generator.

## Model Attributes

The parser recognizes:

```text
@@map(...)
@@index(...)
@@unique(...)
```

These represent table mapping, indexes, and composite unique constraints.

Migration generation includes support for mapped table names, indexes, and unique indexes. Validator coverage for index and unique field names is not complete.

## Relations

Example:

```text
model User {
    id Int @id @default(autoincrement())
    name String
    posts Post[]
}

model Post {
    id Int @id @default(autoincrement())
    title String
    authorId Int

    author User @relation(
        fields: [authorId],
        references: [id]
    )
}
```

Relation arguments recognized by the parser include:

```text
name
fields
references
onDelete
through
pivotFrom
pivotTo
```

Supported referential actions:

```text
Cascade
Restrict
SetNull
NoAction
```

The validator checks relation metadata, referenced fields, reference-key validity, and foreign-key field type compatibility.

**Current limitations:**

- Ordinary relation code generation resolves only single-column `fields` and `references` relationships.
- The validator accepts some composite foreign-key metadata, but that does not mean composite relation code generation is implemented.
- Many-to-many relation metadata is recognized and relation code is generated, but the migration generator does not currently generate many-to-many pivot tables.
- Relation cardinality and all referential-action compatibility rules are not comprehensively validated.

## Many-to-Many Syntax

Example:

```text
model User {
    id Int @id

    roles Role[] @relation(
        through: "user_roles",
        pivotFrom: "user_id",
        pivotTo: "role_id"
    )
}
```

The parser recognizes `through`, `pivotFrom`, and `pivotTo`. The entity generator contains many-to-many relation generation.

**Status:** Code-generation support exists; full pivot-table migration generation and external consumer compilation still need verification.

---

# 2. Lexer

The lexer converts schema source code into tokens.

Implemented token categories include:

- Identifiers
- Strings
- Numbers
- Symbols and punctuation
- Whitespace handling
- `//` line comments
- Invalid-character detection
- Unterminated-string errors

**Current limitations:**

- String escape sequences such as `\"` are not explicitly supported.
- The lexer collects characters that resemble a number; it does not independently guarantee that the token is a valid numeric literal.
- Block comments are not supported by the inspected lexer.

---

# 3. Parser

The parser converts tokens into the Schema AST.

Implemented syntax includes:

- Models and fields
- Scalar types and model references
- Nullable fields and array syntax
- Field attributes
- Model attributes
- Default values
- Relation arguments
- Referential actions
- Many-to-many relation metadata

The parser recognizes the current V0.1 schema syntax. Semantic correctness and support by the generators are handled separately.

---

# 4. Schema AST

The current AST contains:

```text
Schema
Model
Field
FieldType
FieldAttribute
DefaultValue
RelationAttribute
ReferentialAction
ModelAttribute
```

All AST types derive `serde::Serialize` and `serde::Deserialize` for snapshot persistence and change detection.

**Status:** Implemented for the current V0.1 schema language.

---

# 5. Validator

The validator checks semantic correctness after parsing.

## Implemented validations

- Duplicate model names
- Duplicate field names
- Unknown model references
- Missing primary keys
- Multiple primary keys
- Relation attributes on non-model fields
- Unknown relation fields
- Unknown relation reference fields
- Mismatched relation field/reference counts
- Incomplete many-to-many metadata
- Mixed many-to-many and ordinary relation metadata
- Foreign-key fields that are themselves model-relation fields
- Foreign-key and referenced-field scalar type mismatches
- Referenced-key validity, including primary keys and supported unique keys

Each model currently requires exactly one field-level `@id`. Composite primary keys are not supported by the migration generator.

## Validation gaps

The following areas still need additional validation or more comprehensive checks:

- Relation cardinality and array-shape consistency
- Default-value compatibility with field types
- All invalid `@unique` usages
- Comprehensive `@@index` and `@@unique` field validation
- Referential-action compatibility with source-field nullability
- Complete foreign-key nullability and array compatibility checks
- All relation metadata and many-to-many target constraints

**Status:** Core V0.1 validation implemented; additional semantic checks remain.

---

# 6. Entity Generator

The entity generator converts a validated Schema AST into Rust source code.

Generated output:

```text
.rustorm/generated/entities.rs
```

The generator writes the output file and creates its parent directory when necessary. Existing output can be overwritten.

## Generated Components

The generator contains support for:

- Model structs (with `serde::Serialize`, `serde::Deserialize`, `sqlx::FromRow`)
- Entity marker structs
- Entity implementations
- Table and column definitions
- Typed field constants
- Scalar Rust types (mapped to wrapper types where applicable)
- Nullable Rust types
- Mapped table and column names
- Relation metadata and loaders
- Create structs and `InsertData` implementations (with `Serialize`, `Deserialize`)
- Update structs and `UpdateData` implementations (with `Serialize`, `Deserialize`)

## Generated Model Example

For the schema:

```text
model User {
    id Int @id @default(autoincrement())
    name String
}
```

The generated code follows this general shape:

```rust
#[derive(Debug, rustorm::serde::Serialize, rustorm::serde::Deserialize, sqlx::FromRow)]
pub struct UserModel {
    pub id: i32,
    pub name: String,
}

pub struct User;
```

The generator also emits an entity implementation and typed field constants. Exact generated output depends on the schema and the runtime API.

**Verified:** Generated output compiles and works correctly in consumer crates including Axum web applications. Generated model structs include `Serialize` and `Deserialize` derives, enabling direct use with web framework JSON extractors without wrapper types. **Nested eager loading verified end-to-end.**

## Table and Column Mapping

Supported generator behavior includes:

- `@@map` for table names
- `@map` for column names
- Generated table and column metadata
- Basic table-name pluralization when no explicit mapping is provided

The fallback pluralization and snake-case conversion use a shared naming module that ensures consistency between generated entities and migration SQL.

## Relation Generation

The generator contains support for:

- One-to-many relations
- Many-to-one relations
- One-to-one relations
- Many-to-many relations
- Relation key metadata
- Relation loader generation
- Mapped relation columns

**Limitations:**

- Ordinary relation generation currently requires a single local field and a single referenced field.
- Composite relation code generation is not implemented.
- Many-to-many migration pivot-table creation is not implemented.
- Generated relation code has been verified end-to-end with the runtime including nested eager loading.

## Create and Insert Generation

The generator creates `{Model}Create` structs and `InsertData<Model>` implementations.

Current behavior includes:

- Excluding primary-key fields from generated Create structs
- Excluding model-relation fields
- Mapping scalar values to the runtime `BindValue` variants
- Representing nullable values with SQL NULL binding
- Generating insert column and value information
- Including `Serialize` and `Deserialize` derives on Create structs

Scalar-array fields are rejected by CRUD code-generation validation.

Fields with defaults are not universally made optional in Create structs. Application-managed defaults such as CUIDs and identifiers require additional consideration when designing the application-facing creation API.

## Update Generation

The generator creates `{Model}Update` structs and `UpdateData<Model>` implementations.

The intended distinction is:

- Non-nullable fields use `Option<T>` to represent unchanged versus updated values.
- Nullable fields use `Option<Option<T>>` to distinguish unchanged, set-to-value, and set-to-NULL.

Primary-key and relation fields are excluded from the generated update payload.

Update structs include `Serialize` and `Deserialize` derives.

**Verification status:** Unit tests cover generated output patterns. End-to-end compilation and runtime behavior have been verified in consumer projects including Axum applications with nested eager loading.

## Generated Code Dependencies

Generated output references the RustORM runtime and supporting crates, including:

```text
rustorm
sqlx
serde
```

Wrapper types (`rustorm::DateTime`, `rustorm::Decimal`, `rustorm::JsonValue`) are provided by the runtime, eliminating the need for consumers to depend directly on `chrono`, `rust_decimal`, or `serde_json`.

The consuming project must declare compatible dependencies and use runtime APIs matching the generated code. The CLI itself generates source code as text.

---

# 7. Migration System

The migration system handles initial schema generation, incremental schema diffing, migration tracking, and multi-file sequential execution.

## Initial Migration Generation

Default output:

```text
.rustorm/migrations/0001_initial.sql
```

### Implemented SQL Generation

- `CREATE TABLE IF NOT EXISTS` (idempotent)
- Scalar column definitions
- Single-column primary keys
- Integer auto-increment columns using `SERIAL`
- `NOT NULL` for non-nullable fields
- Supported scalar SQL types
- Field-level unique constraints
- Supported default values
- Mapped table names (`@@map`)
- Mapped column names (`@map`)
- `CREATE INDEX`
- `CREATE UNIQUE INDEX`
- Foreign-key constraints
- Supported `ON DELETE` actions
- Single-column and composite foreign-key SQL metadata where supported by relation definitions
- Quoted SQL identifiers with embedded quote escaping
- `_rustorm_migrations` tracking table (auto-created)

### Schema-to-SQL Type Mapping

| Schema type | Generated SQL type |
|---|---|
| `Int` | `INTEGER` |
| `Int @default(autoincrement())` | `SERIAL` |
| `String` | `TEXT` |
| `Boolean` | `BOOLEAN` |
| `Float` | `DOUBLE PRECISION` |
| `DateTime` | `TIMESTAMP` |
| `Decimal` | `NUMERIC` |
| `Json` | `JSONB` |

Array syntax is represented in SQL type generation by appending `[]`, but the complete support of array fields across validation, migration generation, and CRUD generation is not guaranteed.

Application-managed defaults such as CUID and identifier generation are not translated into equivalent SQL defaults by the current migration generator.

## Migration Tracking

Every migration run automatically manages a `_rustorm_migrations` tracking table:

```sql
CREATE TABLE IF NOT EXISTS _rustorm_migrations (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

Tracking behavior:

- **Checksum calculation:** Each migration file's SQL content is hashed (DefaultHasher) to produce a checksum
- **Skip logic:** If a migration with the same name and matching checksum already exists in the tracking table, it is skipped
- **Drift detection:** If a migration with the same name but different checksum is found, a clear error is returned indicating schema drift
- **Recording:** After successful execution, each migration is recorded in the tracking table with its name and checksum
- **Idempotency:** Running `migrate --apply` multiple times with the same schema produces no side effects

**Verified:** Tracking system tested end-to-end — fresh apply, re-apply skip, and drift detection all working.

## Schema Diffing (Incremental Migrations)

When `rustorm dev` detects a schema change, it compares the current schema against the saved snapshot (`snapshot.json`) and generates an incremental migration file.

### Supported Diff Operations

- **ADD COLUMN:** New fields in existing models generate `ALTER TABLE ... ADD COLUMN` statements
- **DROP COLUMN:** Removed fields from existing models generate `ALTER TABLE ... DROP COLUMN IF EXISTS` statements
- **New tables:** Detected and noted in diff output (full CREATE TABLE handled by regeneration)
- **Dropped tables:** Generate `DROP TABLE IF EXISTS ... CASCADE` statements

### Auto-Numbering

Diff migration files are automatically numbered based on existing migration files:

```text
0001_initial.sql
0002_schema_update.sql   ← First schema change
0003_schema_update.sql   ← Second schema change
0004_...                 ← And so on
```

The numbering logic scans all existing `.sql` files in the migrations directory, finds the highest number, and increments it. This prevents duplicate numbering and maintains correct sequential ordering.

### Snapshot-Based Change Detection

- Snapshots are stored as serialized Schema AST in `snapshot.json`
- On each schema save, the old snapshot is loaded and compared against the new parsed schema
- Only actual structural changes trigger diff migration generation
- Unchanged schemas skip diff generation entirely
- Snapshot is updated after each successful regeneration

**Verified:** Schema diffing tested — field addition generates correct ALTER TABLE, auto-numbering assigns correct sequence numbers, duplicate migrations are prevented.

## Migration Execution

The `migrate --apply` command executes migrations against a PostgreSQL database.

### Multi-File Sequential Execution

Unlike single-file execution, `migrate --apply` now:

1. Scans all `.sql` files in the migrations directory
2. Sorts them alphabetically by filename
3. For each file:
   - Calculates checksum
   - Checks tracking table for existing record
   - Skips if already applied with matching checksum
   - Returns error if checksum mismatch (schema drift)
   - Executes SQL if not yet applied
   - Records in tracking table after successful execution
4. Lists all tables in the database after completion

### Database URL Resolution

**Priority order:**

1. `--database` CLI argument
2. `DATABASE_URL` environment variable
3. `.rustorm/.env` file
4. `.rustorm/config.toml` file

Password is masked in console output for security.

### Execution Behavior

- Ensures `_rustorm_migrations` tracking table exists before processing
- Processes all migration files sequentially
- Reports per-file status (skip / apply / error)
- Lists created tables on success
- Reports connection and execution errors clearly

**Verified:** Multi-file migration execution tested end-to-end against PostgreSQL — initial migration, incremental diff migration, and re-run skip all working correctly.

## Not Implemented Yet

- Column type changes (ALTER COLUMN TYPE)
- Column rename detection
- Index additions/removals through schema diffs
- Many-to-many pivot-table creation in diffs
- Migration status reporting command
- Automatic rollback
- Development and reset migration workflows
- Constraint modifications through diffs

---

# 8. CLI Commands

The executable is named `rustorm`.

## Help

```powershell
rustorm --help
```

The CLI also supports `-h`.

## Version

```powershell
rustorm --version
```

The CLI also supports `-V`.

## Initialize a Project

```powershell
rustorm init
```

Creates the `.rustorm/` directory structure with:

- `schema.rustorm` — Template schema file
- `.env` — Database connection string (edit this)
- `config.toml` — Structured configuration
- `.gitignore` — Protects `.env` from git
- `generated/` — Output directory for entities
- `migrations/` — Output directory for migration SQL

## Watch Mode (Development)

```powershell
rustorm dev
```

Starts a file watcher on `.rustorm/schema.rustorm`:

- Performs initial generation on start
- Auto-regenerates entities and migrations on schema change
- Uses snapshot-based change detection to skip unnecessary regeneration
- Generates incremental diff migrations when schema changes are detected
- Auto-numbers diff migration files sequentially
- Includes 300ms debounce to handle editor save events
- Press `Ctrl+C` to stop

## Validate a Schema

```powershell
rustorm validate rust.schema
```

Validates the schema through the lexer, parser, and validator.

## Generate Rust Entities

```powershell
rustorm generate rust.schema
```

Writes:

```text
generated/entities.rs
```

The current generator output path is fixed to `generated/entities.rs`.

## Generate Initial Migration SQL

```powershell
rustorm migrate rust.schema
```

Writes:

```text
migrations/0001_initial.sql
```

To specify another output path:

```powershell
rustorm migrate rust.schema migrations/custom_initial.sql
```

The output directory is created if necessary. An existing output file can be overwritten.

## Apply Migrations to Database

```powershell
rustorm migrate --apply
```

Scans all `.sql` files in the migrations directory, checks each against the tracking table, and applies only pending migrations sequentially.

With explicit database URL:

```powershell
rustorm migrate --apply --database postgres://postgres:admin@localhost/mydb
```

Example output:

```text
🚀 Applying migrations to database...
   Database: postgres://postgres:****@localhost/mydb
📋 Found 3 migration file(s)

⏭️  0001_initial.sql already applied. Skipping.
⏭️  0002_schema_update.sql already applied. Skipping.
📝 Applying: 0003_schema_update.sql
✅ 0003_schema_update.sql applied successfully!

Tables in database:
  📋 _rustorm_migrations
  📋 posts
  📋 users
```

## Default Behavior

When no arguments are provided, the CLI defaults to generating entities from:

```text
rust.schema
```

Passing a schema file path as the first argument also defaults to entity generation.

---

# 9. Configuration

The CLI supports multiple configuration sources for database connection strings.

**Priority order:** CLI argument > environment variable > `.env` file > `config.toml`

## `.env` File

Generated automatically by `rustorm init`:

```text
# .rustorm/.env
# ⚠️ Do NOT commit this file to git!
DATABASE_URL=postgres://postgres:admin@localhost/rustorm_gen
```

Protected by `.gitignore` to prevent credential leaks.

## `config.toml`

Generated automatically by `rustorm init`:

```toml
[database]
url = "postgres://postgres:admin@localhost/rustorm_gen"
```

Supports structured configuration for future extensibility.

## Environment Variable

Standard `DATABASE_URL` environment variable is supported:

```powershell
$env:DATABASE_URL = "postgres://postgres:admin@localhost/mydb"
```

## CLI Argument

```powershell
rustorm migrate --apply --database postgres://...
```

Password is masked in console output for security.

---

# 10. Project Integration

A typical CLI project layout after `rustorm init`:

```text
my-project/
├── Cargo.toml
├── src/
│   └── main.rs
└── .rustorm/
    ├── schema.rustorm      ← Edit your schema here
    ├── .env                ← Database URL (edit this)
    ├── config.toml         ← Advanced configuration
    ├── .gitignore          ← Protects .env from git
    ├── generated/
    │   └── entities.rs     ← Auto-generated (do not edit)
    └── migrations/
        ├── 0001_initial.sql
        ├── 0002_schema_update.sql
        ├── 0003_...
        └── snapshot.json   ← Change detection state
```

Generated Rust code is intended to be consumed by an application using the RustORM runtime.

## Consumer Project Setup

Add to your `Cargo.toml`:

```toml
[dependencies]
rustorm = { path = "../Rust-ORM/rustorm" }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

Include generated entities in your application:

```rust
mod entities {
    include!("../.rustorm/generated/entities.rs");
}

use entities::{User, UserCreate};
use rustorm::Entity;
```

**Verified:** End-to-end integration has been tested with Axum web applications including nested eager loading. Generated types work directly with Axum's `Json` extractor without wrapper types.

---

# 11. Testing and Verification

## Current Test Status

The latest reported CLI test checkpoint is:

```text
61 passed
0 failed
```

The latest reported Clippy verification is:

```powershell
cargo clippy --all-targets --all-features -- -D warnings
```

Result:

```text
Clippy completed without warnings or errors.
```

These results are based on the latest reported local run and should be refreshed after subsequent code changes.

## Test Coverage Areas

- Lexer tokenization and error handling
- Parser syntax and AST construction
- Schema validation
- Model and field constraints
- Relation metadata and referenced keys
- Foreign-key scalar type compatibility
- Entity and relation code generation
- Create and update code generation
- SQL migration generation
- Identifier quoting and mapped names
- Foreign-key SQL generation
- Snapshot-based change detection
- Serde derive generation
- Migration tracking table creation
- Checksum-based skip logic
- Schema diffing (ADD/DROP COLUMN)
- Multi-file migration execution
- Auto-numbering of diff migrations

## End-to-End Verification

**✅ VERIFIED** — Full workflow tested:

```text
rustorm init
      ↓
.rustorm/schema.rustorm (edit schema)
      ↓
rustorm dev (watch mode → auto-regenerate + diff generation)
      ↓
.rustorm/generated/entities.rs
.rustorm/migrations/0001_initial.sql
.rustorm/migrations/0002_schema_update.sql (auto-generated diff)
      ↓
rustorm migrate --apply (multi-file, tracking-aware)
      ↓
PostgreSQL: CREATE / ALTER / READ / UPDATE / DELETE
      ↓
Re-run migrate --apply → all migrations skipped (checksum match)
      ↓
Axum API serving JSON responses with nested eager loading
      ↓
✅ ALL OPERATIONS PASSED
```

Priority verification tasks completed:

1. ✅ Generate entities from schema.
2. ✅ Compile the generated output inside a real consumer crate.
3. ✅ Compile with relations schema.
4. ✅ Compilation tests for Create/InsertData and Update/UpdateData implementations.
5. ✅ Verify relation loading against the actual runtime.
6. ✅ Verify generated migration SQL against a PostgreSQL database.
7. ✅ CLI end-to-end tests for `validate`, `generate`, `migrate`, `init`, and `dev`.
8. ✅ Test error handling and overwriting behavior for generated files.
9. ✅ Migration tracking table auto-creation and recording.
10. ✅ Checksum-based skip and drift detection.
11. ✅ Schema diffing generates correct ALTER TABLE statements.
12. ✅ Multi-file sequential migration execution.
13. ✅ Auto-numbering assigns correct sequence numbers to diff migrations.
14. ✅ **Nested eager loading verified in Axum (User → Posts → Comments).**
15. ✅ **Wrapper types (DateTime) verified in generated entities.**
16. ✅ **Send + Sync compatibility verified with Axum handlers.**

---

# 12. Development Roadmap

## Phase 1 — Verify Generated Code

- [x] Inspect the current schema lexer, parser, AST, validator, and generators.
- [x] Implement core Create and Update code-generation paths.
- [x] Implement initial SQL migration generation.
- [x] Compile generated output in an independent consumer crate.
- [x] Verify generated CRUD implementations against the current runtime.
- [x] Verify relation code generation end to end.
- [x] Verify initial migration SQL against PostgreSQL.

## Phase 2 — Improve Schema and Generator Reliability

- [ ] Validate default values against field types.
- [ ] Improve `@@index` and `@@unique` field validation.
- [ ] Validate relation cardinality and array-shape consistency.
- [ ] Validate `SetNull` and other referential-action compatibility.
- [ ] Resolve composite-relation code-generation limitations.
- [ ] Review scalar-array support and its restrictions.
- [x] Improve naming and pluralization behavior (shared naming module).
- [ ] Ensure generated code handles defaults consistently.
- [ ] Auto-generate `#[sqlx(rename)]` for `@map` fields.
- [ ] Smart `#[serde(skip)]` for back-reference relation fields.
- [ ] Exclude `@default(now())` fields from Create structs.

## Phase 3 — Complete CLI Workflows

- [x] `validate`
- [x] `generate`
- [x] Initial SQL generation through `migrate`
- [x] Help and version arguments
- [x] `init` command (project scaffolding)
- [x] `dev` command (watch mode with auto-regeneration)
- [x] `migrate --apply` (direct database execution)
- [x] `.env` and `config.toml` configuration support
- [x] `.gitignore` generation
- [x] Snapshot-based change detection
- [x] Migration tracking table
- [x] Checksum-based skip and drift detection
- [x] Multi-file sequential migration execution
- [x] Auto-numbering for diff migrations
- [ ] Add CLI end-to-end tests.
- [ ] Improve error messages and failure cases.
- [x] Review safe handling of existing generated files.
- [ ] Document command behavior and output paths.

## Phase 4 — Migration System

- [x] Generate initial `CREATE TABLE` SQL.
- [x] Generate supported indexes and foreign-key constraints.
- [x] Apply migrations to live database (`--apply`).
- [x] Migration tracking table (`_rustorm_migrations`).
- [x] Schema-diff migrations (ADD/DROP COLUMN).
- [x] Auto-numbered incremental migration files.
- [x] Checksum-based idempotent execution.
- [x] Schema drift detection.
- [ ] Column type changes (ALTER COLUMN TYPE).
- [ ] Column rename detection.
- [ ] Index additions/removals through diffs.
- [ ] Add migration status reporting command.
- [ ] Design rollback and reset workflows.
- [ ] Generate many-to-many pivot tables.

## Phase 5 — Release Readiness

- [x] Add a sample application using the runtime (Axum example).
- [x] Add generated-code compilation tests.
- [x] Add database integration tests.
- [x] Verify nested eager loading end-to-end.
- [ ] Document supported and unsupported schema features.
- [ ] Document dependency and runtime-version requirements.
- [x] Prepare a reproducible V0.1 example workflow.
- [ ] Prepare crates.io publication.

---

# 13. Current Milestone

**V0.1 — Schema → Validated AST → Generated Rust → Tracked Migrations → Schema Diffing → Database → Web API with Nested Eager Loading**

## Implemented

- [x] Lexer
- [x] Parser
- [x] Schema AST (with serde Serialize/Deserialize)
- [x] Core semantic validation
- [x] Model and entity generation
- [x] Typed field generation
- [x] Relation code-generation paths
- [x] Create struct generation
- [x] `InsertData` generation
- [x] Update struct generation
- [x] `UpdateData` generation
- [x] Serde `Serialize`/`Deserialize` derives on generated types
- [x] Initial SQL migration generation (`CREATE TABLE IF NOT EXISTS`)
- [x] Migration tracking table (`_rustorm_migrations`)
- [x] Checksum-based migration skip logic
- [x] Schema drift detection (checksum mismatch error)
- [x] Schema diffing (ADD COLUMN, DROP COLUMN)
- [x] Auto-numbered incremental migration files
- [x] Multi-file sequential migration execution
- [x] Snapshot-based change detection
- [x] Migration execution (`--apply`)
- [x] `validate` command
- [x] `generate` command
- [x] `migrate` command for SQL file generation
- [x] `init` command (project scaffolding)
- [x] `dev` command (watch mode with diff generation)
- [x] `.env` file support
- [x] `config.toml` support
- [x] `.gitignore` generation
- [x] Shared naming module (table/column conventions)
- [x] Help and version arguments
- [x] 61 tests passing at the latest reported checkpoint
- [x] Clippy clean at the latest reported checkpoint
- [x] End-to-end verification with Axum example
- [x] Migration tracking verified end-to-end
- [x] Schema diffing verified end-to-end
- [x] Multi-file execution verified end-to-end
- [x] **Nested eager loading verified (2-level: User → Posts → Comments)**
- [x] **Wrapper types in generated entities (DateTime, Decimal, JsonValue)**
- [x] **Send + Sync async compatibility verified**

## Remaining Before a Reliable V0.1 Release

- [ ] Resolve or clearly document unsupported schema combinations.
- [ ] Complete the user-facing usage guide.
- [ ] Prepare crates.io publication.
- [ ] ORM-level error types.
- [ ] Generator automation (`@map` → `#[sqlx(rename)]`, smart `#[serde(skip)]`, `created_at` exclusion).

The implementation is beyond basic entity generation. Migration tracking, schema diffing, multi-file execution, generated-code compilation, runtime integration, database behavior, nested eager loading, and web framework integration have all been verified end-to-end.

---

# 14. Axum Integration Example

Generated entities include `Serialize` and `Deserialize` derives, enabling direct use with Axum including nested eager loading:

```rust
mod entities {
    include!("../.rustorm/generated/entities.rs");
}

use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use entities::{User, Post};
use rustorm::Entity;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Clone)]
struct AppState { db: PgPool }

// DTO pattern for clean API responses
#[derive(Serialize)]
struct PostWithCommentsResponse {
    #[serde(flatten)]
    post: PostResponse,
    comments: Vec<CommentResponse>,
}

#[derive(Serialize)]
struct UserWithNestedResponse {
    #[serde(flatten)]
    user: UserResponse,
    posts: Vec<PostWithCommentsResponse>,
}

// 2-level nested eager loading (User → Posts → Comments)
async fn list_users_with_posts_comments(
    State(state): State<AppState>,
) -> Json<Vec<UserWithNestedResponse>> {
    let users = User::find()
        .with_nested(User::posts, |q| q.with(Post::comments))
        .all(&state.db)
        .await
        .unwrap_or_default();

    let response = users.iter().map(|user| UserWithNestedResponse {
        user: UserResponse::from(user),
        posts: user.posts.iter().map(|post| PostWithCommentsResponse {
            post: PostResponse::from(post),
            comments: post.comments.iter().map(CommentResponse::from).collect(),
        }).collect(),
    }).collect();

    Json(response)
}
```

**Verified endpoints:**

| Endpoint | Relation Type | Status |
|---|---|---|
| `GET /users` | Basic CRUD | ✅ |
| `GET /posts/with-user` | N:1 (ManyToOne) | ✅ |
| `GET /users/with-posts` | 1:N (OneToMany) | ✅ |
| `GET /users/with-profile` | 1:1 (OneToOne) | ✅ |
| `GET /users/with-posts-comments` | **2-level nested** | ✅ |

Full working example available at: [`rustorm-example-axum`](https://github.com/Fakhir-Israr-200219/rustorm-example-axum)

---

# 15. Design Principles

RustORM CLI should remain a schema and code-generation toolchain.

```text
CLI
├── Read schema
├── Tokenize and parse
├── Build AST
├── Validate schema
├── Generate Rust code
├── Generate initial SQL
├── Generate incremental diffs (schema diffing)
├── Track applied migrations (checksum-based)
├── Watch for schema changes (dev)
├── Execute migrations sequentially (--apply)
└── Manage configuration (.env / config.toml)

Runtime
├── Connect to databases
├── Build queries
├── Execute database operations
├── Manage entities and relations
├── Nested eager loading (.with / .with_nested)
├── Wrapper types (DateTime, Decimal, JsonValue)
├── Send + Sync async compatibility
└── Handle transactions
```

The CLI should generate code for the existing runtime rather than reimplement runtime behavior.

Features should be marked complete only at the appropriate level: parser support, validation, code generation, compilation, and runtime behavior are distinct milestones.

This separation is central to the long-term design of RustORM.
```

### Key Changes Summary

| Section | Change |
|---|---|
| Header status | Added nested eager loading + Axum integration |
| Runtime Boundary | Added nested eager loading, wrapper types, Send+Sync |
| §1 Scalar Types | Updated DateTime/Decimal/Json → wrapper types |
| §6 Entity Generator | Updated scalar types, dependencies, verified status |
| §11 E2E Verification | Added 3 new ✅ tasks (nested, wrappers, Send+Sync) |
| §12 Phase 2 | Added 3 new generator automation items |
| §12 Phase 5 | Added nested eager loading verification |
| §13 Current Milestone | Title updated, added 3 new ✅ items, remaining updated |
| §14 Axum Example | Replaced with nested eager loading + DTO example |
| §15 Design Principles | Added 3 new runtime responsibilities |
