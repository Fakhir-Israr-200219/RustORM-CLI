# RustORM CLI

RustORM CLI is the schema parser, validator, Rust code generator, and initial SQL migration generator for RustORM.

It aims to bring a Prisma-inspired development workflow to Rust by using a schema file to describe models, fields, and relations, then generating Rust entity code and initial database schema SQL.

The CLI is a separate project from the RustORM runtime.

> **Status:** Early development — V0.1  
> Schema parsing, AST construction, semantic validation, entity and CRUD code generation, and initial SQL migration generation are implemented. Generated-code compilation and full application-level integration still require dedicated end-to-end verification.

---

## Architecture

```text
rust.schema
    |
    v
Lexer
    |
    v
Parser
    |
    v
Schema AST
    |
    v
Validator
    |
    +--------------------+
    |                    |
    v                    v
Entity Generator    Migration Generator
    |                    |
    v                    v
generated/          migrations/
entities.rs         0001_initial.sql
```

## Runtime Boundary

```text
RustORM CLI
    |
    +-- Schema parsing
    +-- Schema validation
    +-- Rust code generation
    +-- Initial migration SQL generation
              |
              v
       RustORM Runtime
              |
              +-- Query building
              +-- Database execution
              +-- Entity operations
              +-- Runtime relations
              +-- Transactions
```

The CLI is a toolchain, not a second ORM runtime. Generated Rust code targets the existing RustORM runtime and its supporting dependencies.

The `migrate` command currently generates an initial SQL file. It does not connect to a database or apply migrations.

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
| `DateTime` | `chrono::NaiveDateTime` |
| `Decimal` | `rust_decimal::Decimal` |
| `Json` | `serde_json::Value` |

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

The AST stores model definitions, field types, nullability, array metadata, field attributes, and model-level attributes.

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
generated/entities.rs
```

The generator writes the output file and creates its parent directory when necessary. Existing output can be overwritten.

## Generated Components

The generator contains support for:

- Model structs
- Entity marker structs
- Entity implementations
- Table and column definitions
- Typed field constants
- Scalar Rust types
- Nullable Rust types
- Mapped table and column names
- Relation metadata and loaders
- Create structs and `InsertData` implementations
- Update structs and `UpdateData` implementations

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
#[derive(Debug, sqlx::FromRow)]
pub struct UserModel {
    pub id: i32,
    pub name: String,
}

pub struct User;
```

The generator also emits an entity implementation and typed field constants. Exact generated output depends on the schema and the runtime API.

**Important:** This is an illustrative excerpt, not a guarantee that the excerpt alone compiles. Generated output must be tested against a real consumer crate using compatible runtime and dependency versions.

## Table and Column Mapping

Supported generator behavior includes:

- `@@map` for table names
- `@map` for column names
- Generated table and column metadata
- Basic table-name pluralization when no explicit mapping is provided

The fallback pluralization and snake-case conversion are intentionally simple and may not handle every naming convention correctly.

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
- Generated relation code still needs end-to-end compilation and runtime verification.

## Create and Insert Generation

The generator creates `{Model}Create` structs and `InsertData<Model>` implementations.

Current behavior includes:

- Excluding primary-key fields from generated Create structs
- Excluding model-relation fields
- Mapping scalar values to the runtime `BindValue` variants
- Representing nullable values with SQL NULL binding
- Generating insert column and value information

Scalar-array fields are rejected by CRUD code-generation validation.

Fields with defaults are not universally made optional in Create structs. Application-managed defaults such as CUIDs and identifiers require additional consideration when designing the application-facing creation API.

## Update Generation

The generator creates `{Model}Update` structs and `UpdateData<Model>` implementations.

The intended distinction is:

- Non-nullable fields use `Option<T>` to represent unchanged versus updated values.
- Nullable fields use `Option<Option<T>>` to distinguish unchanged, set-to-value, and set-to-NULL.

Primary-key and relation fields are excluded from the generated update payload.

**Verification status:** Unit tests cover generated output patterns, but a successful string-generation test does not prove that generated code compiles or behaves correctly in a consuming application.

## Generated Code Dependencies

Generated output references the RustORM runtime and supporting crates, including:

```text
rustorm
sqlx
chrono
rust_decimal
serde_json
```

The consuming project must declare compatible dependencies and use runtime APIs matching the generated code. The CLI itself has no external dependencies in its current `Cargo.toml`; it generates source code as text.

---

# 7. Migration Generator

The migration generator converts schema information into an initial SQL schema file.

Default output:

```text
migrations/0001_initial.sql
```

The output path can be supplied to the CLI.

## Implemented SQL Generation

- `CREATE TABLE`
- Scalar column definitions
- Single-column primary keys
- Integer auto-increment columns using `SERIAL`
- `NOT NULL` for non-nullable fields
- Supported scalar SQL types
- Field-level unique constraints
- Supported default values
- Mapped table names
- Mapped column names
- `CREATE INDEX`
- `CREATE UNIQUE INDEX`
- Foreign-key constraints
- Supported `ON DELETE` actions
- Single-column and composite foreign-key SQL metadata where supported by relation definitions
- Quoted SQL identifiers with embedded quote escaping

## Schema-to-SQL Type Mapping

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

## Not Implemented Yet

- Schema diffing
- `ALTER TABLE` migrations
- Adding, dropping, or renaming columns
- Adding or removing indexes through schema diffs
- Many-to-many pivot-table creation
- Migration history tracking
- Migration status command
- Applying SQL to a live database
- Automatic rollback
- Development and reset migration workflows

**Important:** `rustorm migrate` generates an initial SQL file. It does not execute that SQL against PostgreSQL or another database.

---

# 8. CLI Commands

The executable is named `rustorm`.

## Help

```powershell
cargo run -- --help
```

The CLI also supports `-h`.

## Version

```powershell
cargo run -- --version
```

The CLI also supports `-V`.

## Validate a Schema

```powershell
cargo run -- validate rust.schema
```

Validates the schema through the lexer, parser, and validator.

## Generate Rust Entities

```powershell
cargo run -- generate rust.schema
```

Writes:

```text
generated/entities.rs
```

The current generator output path is fixed to `generated/entities.rs`.

## Generate Initial Migration SQL

```powershell
cargo run -- migrate rust.schema
```

Writes:

```text
migrations/0001_initial.sql
```

To specify another output path:

```powershell
cargo run -- migrate rust.schema migrations/custom_initial.sql
```

The output directory is created if necessary. An existing output file can be overwritten.

## Default Behavior

When no arguments are provided, the CLI defaults to generating entities from:

```text
rust.schema
```

Passing a schema file path as the first argument also defaults to entity generation.

**Not currently implemented:** `migrate dev`, `migrate status`, `migrate reset`, database connection management, and automatic migration application.

---

# 9. Project Integration

A typical CLI project layout is:

```text
RustORM-CLI/
├── Cargo.toml
├── rust.schema
├── rust.relations.schema
├── src/
│   ├── main.rs
│   ├── lexer.rs
│   ├── parser.rs
│   ├── ast.rs
│   ├── validator.rs
│   ├── generator.rs
│   └── migration.rs
├── generated/
│   └── entities.rs
└── migrations/
    └── 0001_initial.sql
```

Generated Rust code is intended to be consumed by an application using the RustORM runtime.

Application-level integration still requires verification of:

- Compatible runtime APIs
- Required dependencies
- Generated module declarations
- Generated-code compilation
- Database compatibility
- End-to-end CRUD and relation behavior

The CLI currently does not automatically discover and configure a consumer's Cargo project or integrate generated modules into an application.

---

# 10. Testing and Verification

## Current Test Status

The latest reported CLI test checkpoint is:

```text
59 passed
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

## Verification Still Needed

Passing unit tests does not by itself establish that the generated source compiles in an independent consumer project.

Priority verification tasks:

1. Generate entities from `rust.schema`.
2. Compile the generated output inside a real consumer crate.
3. Repeat with `rust.relations.schema`.
4. Add compilation tests for Create/InsertData and Update/UpdateData implementations.
5. Verify relation loading against the actual runtime.
6. Verify generated migration SQL against a PostgreSQL database.
7. Add CLI end-to-end tests for `validate`, `generate`, and `migrate`.
8. Test error handling and overwriting behavior for generated files.

---

# 11. Development Roadmap

## Phase 1 — Verify Generated Code

- [x] Inspect the current schema lexer, parser, AST, validator, and generators.
- [x] Implement core Create and Update code-generation paths.
- [x] Implement initial SQL migration generation.
- [x] Compile generated output in an independent consumer crate.
- [ ] Verify generated CRUD implementations against the current runtime.
- [ ] Verify relation code generation end to end.
- [ ] Verify initial migration SQL against PostgreSQL.

## Phase 2 — Improve Schema and Generator Reliability

- [ ] Validate default values against field types.
- [ ] Improve `@@index` and `@@unique` field validation.
- [ ] Validate relation cardinality and array-shape consistency.
- [ ] Validate `SetNull` and other referential-action compatibility.
- [ ] Resolve composite-relation code-generation limitations.
- [ ] Review scalar-array support and its restrictions.
- [ ] Improve naming and pluralization behavior.
- [ ] Ensure generated code handles defaults consistently.

## Phase 3 — Complete CLI Workflows

- [x] `validate`
- [x] `generate`
- [x] Initial SQL generation through `migrate`
- [x] Help and version arguments
- [ ] Add CLI end-to-end tests.
- [ ] Improve error messages and failure cases.
- [ ] Review safe handling of existing generated files.
- [ ] Document command behavior and output paths.

## Phase 4 — Migration System

- [x] Generate initial `CREATE TABLE` SQL.
- [x] Generate supported indexes and foreign-key constraints.
- [ ] Generate schema-diff migrations.
- [ ] Add `ALTER TABLE` support.
- [ ] Add migration history.
- [ ] Add migration status reporting.
- [ ] Add safe migration application.
- [ ] Design rollback and reset workflows.
- [ ] Generate many-to-many pivot tables.

## Phase 5 — Release Readiness

- [ ] Add a sample application using the runtime.
- [ ] Add generated-code compilation tests.
- [ ] Add database integration tests.
- [ ] Document supported and unsupported schema features.
- [ ] Document dependency and runtime-version requirements.
- [ ] Prepare a reproducible V0.1 example workflow.

---

# 12. Current Milestone

**V0.1 — Schema → Validated AST → Generated Rust and Initial SQL**

## Implemented

- [x] Lexer
- [x] Parser
- [x] Schema AST
- [x] Core semantic validation
- [x] Model and entity generation
- [x] Typed field generation
- [x] Relation code-generation paths
- [x] Create struct generation
- [x] `InsertData` generation
- [x] Update struct generation
- [x] `UpdateData` generation
- [x] Initial SQL migration generation
- [x] `validate` command
- [x] `generate` command
- [x] `migrate` command for SQL file generation
- [x] Help and version arguments
- [x] 59 tests passing at the latest reported checkpoint
- [x] Clippy clean at the latest reported checkpoint

## Remaining Before a Reliable V0.1 Release

- [ ] Verify generated Rust code compiles in a consumer project.
- [ ] Verify CRUD and relation behavior against the runtime.
- [ ] Verify migration SQL against a real database.
- [ ] Add CLI end-to-end tests.
- [ ] Resolve or clearly document unsupported schema combinations.
- [ ] Complete the user-facing usage guide.

The implementation is beyond basic entity generation, but it should not be considered fully production-ready until generated-code compilation, runtime integration, and database behavior have been verified.

---

# 13. Design Principles

RustORM CLI should remain a schema and code-generation toolchain.

```text
CLI
├── Read schema
├── Tokenize and parse
├── Build AST
├── Validate schema
├── Generate Rust code
└── Generate initial SQL

Runtime
├── Connect to databases
├── Build queries
├── Execute database operations
├── Manage entities and relations
└── Handle transactions
```

The CLI should generate code for the existing runtime rather than reimplement runtime behavior.

Features should be marked complete only at the appropriate level: parser support, validation, code generation, compilation, and runtime behavior are distinct milestones.

This separation is central to the long-term design of RustORM.